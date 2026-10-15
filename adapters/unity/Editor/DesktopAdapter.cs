#if UNITY_EDITOR_WIN
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using Microsoft.Win32.SafeHandles;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using Object = UnityEngine.Object;

namespace Bebekon.SmartComputer.Editor
{
    [InitializeOnLoad]
    public static class DesktopAdapter
    {
        private const int MaxBytes = 4 * 1024 * 1024;
        private static readonly ConcurrentQueue<Job> Jobs = new ConcurrentQueue<Job>();
        private static readonly ConcurrentQueue<Log> Logs = new ConcurrentQueue<Log>();
        private static readonly Regex Secret = new Regex(@"sk-[A-Za-z0-9_-]{12,}|(?i:bearer)\s+[A-Za-z0-9._-]{12,}", RegexOptions.Compiled);
        private static readonly int EditorPid = Process.GetCurrentProcess().Id;
        private static readonly string UserSid = Native.CurrentSid();
        private static volatile bool enabled;
        private static Thread listener;
        private static NamedPipeServerStream current;
        private static string trustedHost;
        private static int queued;
        public static readonly string Generation = Guid.NewGuid().ToString();
        private static readonly Dictionary<Object, int> References = new Dictionary<Object, int>();
        private static readonly Dictionary<int, Object> Objects = new Dictionary<int, Object>();
        private static readonly Queue<int> Order = new Queue<int>();
        private static int nextId;
        public static int Reference(Object value)
        {
            if (!value) return 0;
            if (References.TryGetValue(value, out var existing)) return existing;
            var id = ++nextId; References[value] = id; Objects[id] = value; Order.Enqueue(id);
            while (Order.Count > 1024) { var old = Order.Dequeue(); References.Remove(Objects[old]); Objects.Remove(old); }
            return id;
        }

        [Serializable] public sealed class Configuration { public string executable; }
        [Serializable] public sealed class Request
        {
            public string op, view, goal;
            public int limit = 30;
            public bool visual;
            public Command command;
        }
        [Serializable] public sealed class Command
        {
            public string kind, name, generation;
            public int object_id;
            public bool active;
            public float[] position, rotation, scale;
        }
        [Serializable] private sealed class Reply { public State state; public string image, error; public bool verified; public int width, height; }
        [Serializable] public sealed class Entry { public int id, parent; public string name, type; public bool active; }
        [Serializable] public sealed class Property { public string name, type, value; }
        [Serializable] public sealed class Log { public string kind, message; }
        [Serializable] private sealed class State
        {
            public string unity_version, scene, view, generation;
            public bool playing, paused, compiling, scene_dirty;
            public Entry[] objects;
            public Property[] properties;
            public Log[] console;
        }
        private sealed class Job
        {
            public Request request;
            public NamedPipeServerStream pipe;
            public readonly ManualResetEventSlim done = new ManualResetEventSlim();
            public volatile bool cancelled;
            public string answer;
            public readonly long deadline = Stopwatch.GetTimestamp() + 5 * Stopwatch.Frequency;
            public bool Permitted() { return enabled && !cancelled && Stopwatch.GetTimestamp() < deadline && pipe.IsConnected && Native.PeekNamedPipe(pipe.SafePipeHandle, IntPtr.Zero, 0, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero); }
        }
        static DesktopAdapter()
        {
            EditorApplication.update += Update;
            AssemblyReloadEvents.beforeAssemblyReload += Disable;
            EditorApplication.quitting += Disable;
            Application.logMessageReceivedThreaded += RecordLog;
            // Deliberately opt-in after each Editor launch/domain reload.
        }
        [MenuItem("Tools/Bebekon/Smart Computer/Enable for this Editor")]
        public static void Enable()
        {
            if (enabled) return;
            var configPath = Path.Combine(Application.dataPath, "BebekonSmartComputer/Editor/host.json");
            var config = JsonUtility.FromJson<Configuration>(File.ReadAllText(configPath));
            trustedHost = Path.GetFullPath(config.executable);
            if (!File.Exists(trustedHost)) throw new InvalidOperationException("Smart Computer host is unavailable.");
            enabled = true;
            listener = new Thread(Listen) { IsBackground = true, Name = "Smart Computer Unity IPC" };
            listener.Start();
        }
        [MenuItem("Tools/Bebekon/Smart Computer/Disable")]
        public static void Disable()
        {
            enabled = false;
            try { current?.Dispose(); } catch (IOException) { }
            while (Jobs.TryDequeue(out var job)) { Interlocked.Decrement(ref queued); job.cancelled = true; job.done.Set(); }
            if (listener != null && listener != Thread.CurrentThread) listener.Join(250);
            listener = null;
        }
        private static void RecordLog(string message, string stack, LogType type)
        {
            if (!enabled) return;
            Logs.Enqueue(new Log { kind = type.ToString(), message = Clean(message, 512) });
            while (Logs.Count > 128) Logs.TryDequeue(out _);
        }
        private static string Clean(string value, int max)
        {
            value = Secret.Replace(value ?? "", "[redacted]");
            return value.Length > max ? value.Substring(0, max) : value;
        }
        private static void Listen()
        {
            while (enabled)
            {
                try
                {
                    using (var pipe = Native.CreatePipe("BebekonUnity-" + EditorPid, UserSid))
                    {
                        current = pipe;
                        pipe.WaitForConnection();
                        if (!enabled || !Native.Trusted(pipe, trustedHost, EditorPid, UserSid)) continue;
                        var reader = new BinaryReader(pipe, Encoding.UTF8, true);
                        var count = reader.ReadInt32();
                        if (count < 2 || count > 16384) continue;
                        var bytes = reader.ReadBytes(count);
                        if (bytes.Length != count) continue;
                        var request = JsonUtility.FromJson<Request>(Encoding.UTF8.GetString(bytes));
                        if (request == null || request.limit < 1 || request.limit > 80 || (request.goal?.Length ?? 0) > 512) continue;
                        if (Interlocked.Increment(ref queued) > 4) { Interlocked.Decrement(ref queued); continue; }
                        var job = new Job { request = request, pipe = pipe };
                        Jobs.Enqueue(job);
                        if (!job.done.Wait(5000)) job.cancelled = true;
                        var reply = Encoding.UTF8.GetBytes(job.answer ?? "{\"error\":\"Unity request cancelled or timed out\"}");
                        if (reply.Length > MaxBytes) reply = Encoding.UTF8.GetBytes("{\"error\":\"Unity response is too large\"}");
                        var writer = new BinaryWriter(pipe, Encoding.UTF8, true);
                        writer.Write(reply.Length); writer.Write(reply); writer.Flush();
                    }
                }
                catch (IOException) { }
                catch (ObjectDisposedException) { }
                catch (Exception) { /* Static error only; requests, paths and credentials are never logged. */ }
                finally { current = null; }
            }
        }
        private static void Update()
        {
            if (!Jobs.TryDequeue(out var job)) return;
            Interlocked.Decrement(ref queued);
            if (!job.Permitted()) { job.cancelled = true; job.done.Set(); return; }
            try
            {
                if (job.request.op == "observe" && job.request.visual && job.request.view == "game")
                {
                    if (!EditorApplication.isPlaying || EditorApplication.isPaused) throw new InvalidOperationException("Game frame requires advancing Play Mode; use window capture otherwise.");
                    EndFrameCapture.Request(job.Permitted, texture => CompleteImage(job, texture));
                    return;
                }
                var reply = Execute(job.request);
                job.answer = JsonUtility.ToJson(reply);
            }
            catch (Exception error) { job.answer = JsonUtility.ToJson(new Reply { error = Clean(error.Message, 256) }); }
            finally { if (job.answer != null || job.cancelled) job.done.Set(); }
        }
        private static void CompleteImage(Job job, Texture2D texture)
        {
            try
            {
                if (!job.Permitted()) { job.cancelled = true; return; }
                var result = Encode(texture);
                job.answer = JsonUtility.ToJson(result);
            }
            catch (Exception) { job.answer = "{\"error\":\"Game frame unavailable\"}"; }
            finally { if (texture) Object.DestroyImmediate(texture); job.done.Set(); }
        }
        private static Reply Encode(Texture2D source)
        {
            if (!source || source.width < 1 || source.height < 1 || (long)source.width * source.height > 20000000) throw new InvalidOperationException("Invalid frame size.");
            var factor = Math.Min(1f, 1280f / Math.Max(source.width, source.height));
            var width = Math.Max(1, (int)(source.width * factor)); var height = Math.Max(1, (int)(source.height * factor));
            var target = RenderTexture.GetTemporary(width, height, 0, RenderTextureFormat.ARGB32);
            var before = RenderTexture.active; Texture2D resized = null;
            try
            {
                Graphics.Blit(source, target); RenderTexture.active = target;
                resized = new Texture2D(width, height, TextureFormat.RGB24, false);
                resized.ReadPixels(new Rect(0, 0, width, height), 0, 0); resized.Apply();
                var png = resized.EncodeToPNG();
                if (png.Length > 2 * 1024 * 1024) throw new InvalidOperationException("Frame is too large.");
                return new Reply { image = Convert.ToBase64String(png), width = width, height = height };
            }
            finally { RenderTexture.active = before; if (resized) Object.DestroyImmediate(resized); RenderTexture.ReleaseTemporary(target); }
        }
        private static Reply SceneFrame()
        {
            var scene = SceneView.lastActiveSceneView;
            if (!scene || !scene.camera) throw new InvalidOperationException("Open a Scene View first.");
            var camera = scene.camera; var original = camera.targetTexture; var active = RenderTexture.active;
            var target = RenderTexture.GetTemporary(1280, 720, 24, RenderTextureFormat.ARGB32); Texture2D texture = null;
            try
            {
                camera.targetTexture = target; camera.Render(); RenderTexture.active = target;
                texture = new Texture2D(1280, 720, TextureFormat.RGB24, false); texture.ReadPixels(new Rect(0, 0, 1280, 720), 0, 0); texture.Apply();
                return Encode(texture);
            }
            finally { camera.targetTexture = original; RenderTexture.active = active; if (texture) Object.DestroyImmediate(texture); RenderTexture.ReleaseTemporary(target); }
        }
        private static Entry Describe(Object value)
        {
            var go = value as GameObject;
            return new Entry { id = Reference(value), name = Clean(value.name, 128), type = value.GetType().Name, parent = go && go.transform.parent ? Reference(go.transform.parent.gameObject) : 0, active = !go || go.activeSelf };
        }
        public static bool Apply(Command command)
        {
            if (command == null || EditorApplication.isPlayingOrWillChangePlaymode || EditorApplication.isCompiling) throw new InvalidOperationException("Editor is not ready for scene edits.");
            if (command.generation != Generation || !Objects.TryGetValue(command.object_id, out var reference)) throw new InvalidOperationException("Stale object reference; observe this Editor again.");
            var go = reference as GameObject;
            if (!go || EditorUtility.IsPersistent(go) || !go.scene.IsValid() || !go.scene.isLoaded || go.hideFlags != HideFlags.None) throw new InvalidOperationException("Only editable loaded scene objects are allowed.");
            switch (command.kind)
            {
                case "select": Selection.activeGameObject = go; return Selection.activeGameObject == go;
                case "rename":
                    if (string.IsNullOrWhiteSpace(command.name) || command.name.Length > 128) throw new InvalidOperationException("Invalid object name.");
                    Undo.RecordObject(go, "Smart Computer Rename"); go.name = command.name; Changed(go); return go.name == command.name;
                case "set_active": Undo.RecordObject(go, "Smart Computer Active"); go.SetActive(command.active); Changed(go); return go.activeSelf == command.active;
                case "set_transform":
                    CheckVector(command.position); CheckVector(command.rotation); CheckVector(command.scale);
                    if (command.position == null && command.rotation == null && command.scale == null) throw new InvalidOperationException("Specify a transform change.");
                    var transform = go.transform; Undo.RecordObject(transform, "Smart Computer Transform");
                    if (command.position != null) transform.localPosition = Vector(command.position);
                    if (command.rotation != null) transform.localEulerAngles = Vector(command.rotation);
                    if (command.scale != null) transform.localScale = Vector(command.scale);
                    PrefabUtility.RecordPrefabInstancePropertyModifications(transform); EditorSceneManager.MarkSceneDirty(go.scene);
                    return (command.position == null || transform.localPosition == Vector(command.position)) && (command.scale == null || transform.localScale == Vector(command.scale)) && (command.rotation == null || Quaternion.Angle(transform.localRotation, Quaternion.Euler(Vector(command.rotation))) < .01f);
                default: throw new InvalidOperationException("Unsupported command. Arbitrary C# execution is forbidden.");
            }
        }
        private static void Changed(GameObject go) { PrefabUtility.RecordPrefabInstancePropertyModifications(go); EditorSceneManager.MarkSceneDirty(go.scene); }
        private static Vector3 Vector(float[] value) { return new Vector3(value[0], value[1], value[2]); }
        private static void CheckVector(float[] value) { if (value == null) return; if (value.Length != 3) throw new InvalidOperationException("Invalid vector."); foreach (var f in value) if (float.IsNaN(f) || float.IsInfinity(f) || Math.Abs(f) > 1000000) throw new InvalidOperationException("Invalid vector."); }
        private static Reply Execute(Request request)
        {
            if (request.op == "act") return new Reply { verified = Apply(request.command) };
            if (request.op != "observe") throw new InvalidOperationException("Unsupported request.");
            if (request.visual) { if (request.view == "scene") return SceneFrame(); throw new InvalidOperationException("Use scene or game for adapter images."); }
            var scene = SceneManager.GetActiveScene();
            var state = new State { generation = Generation, unity_version = Application.unityVersion, scene = Clean(scene.name, 128), scene_dirty = scene.isDirty, playing = EditorApplication.isPlaying, paused = EditorApplication.isPaused, compiling = EditorApplication.isCompiling, view = request.view };
            switch (request.view)
            {
                case "state": case "selection":
                    var selection = new List<Entry>(); foreach (var obj in Selection.objects) { if (selection.Count == request.limit) break; selection.Add(Describe(obj)); } state.objects = selection.ToArray(); break;
                case "hierarchy":
                    var found = new List<Entry>(); var queue = new Queue<Transform>();
                    for (var i = 0; i < SceneManager.sceneCount; i++) foreach (var root in SceneManager.GetSceneAt(i).GetRootGameObjects()) queue.Enqueue(root.transform);
                    var scanned = 0;
                    while (queue.Count > 0 && scanned++ < 1000 && found.Count < request.limit)
                    {
                        var transform = queue.Dequeue(); if (string.IsNullOrEmpty(request.goal) || transform.name.IndexOf(request.goal, StringComparison.OrdinalIgnoreCase) >= 0) found.Add(Describe(transform.gameObject));
                        foreach (Transform child in transform) { if (queue.Count < 1000) queue.Enqueue(child); }
                    }
                    state.objects = found.ToArray(); break;
                case "inspector":
                    var objSelected = Selection.activeObject; var properties = new List<Property>();
                    if (objSelected)
                    {
                        var serialized = new SerializedObject(objSelected); var iterator = serialized.GetIterator();
                        for (var visited = 0; visited < 256 && properties.Count < request.limit && iterator.NextVisible(visited == 0); visited++)
                        {
                            if (Regex.IsMatch(iterator.name, "password|secret|token|credential|api.?key", RegexOptions.IgnoreCase)) continue;
                            var value = iterator.propertyType == SerializedPropertyType.String ? iterator.stringValue : iterator.propertyType == SerializedPropertyType.Boolean ? iterator.boolValue.ToString() : iterator.propertyType == SerializedPropertyType.Integer ? iterator.intValue.ToString() : iterator.propertyType == SerializedPropertyType.Float ? iterator.floatValue.ToString(System.Globalization.CultureInfo.InvariantCulture) : iterator.propertyType == SerializedPropertyType.ObjectReference ? iterator.objectReferenceValue ? iterator.objectReferenceValue.name : "null" : iterator.propertyType == SerializedPropertyType.Vector3 ? iterator.vector3Value.ToString() : "";
                            properties.Add(new Property { name = iterator.propertyPath, type = iterator.propertyType.ToString(), value = Clean(value, 256) });
                        }
                    }
                    state.properties = properties.ToArray(); break;
                case "console": var logs = Logs.ToArray(); var take = Math.Min(request.limit, logs.Length); state.console = new Log[take]; Array.Copy(logs, logs.Length - take, state.console, 0, take); break;
                default: throw new InvalidOperationException("Unsupported adapter view.");
            }
            return new Reply { state = state };
        }
        private static class Native
        {
            [StructLayout(LayoutKind.Sequential)] private struct Attributes { public int length; public IntPtr descriptor; public int inherit; }
            [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] private struct ProcessEntry { public uint size, usage, pid; public IntPtr heap; public uint module, threads, parent; public int priority; public uint flags; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string exe; }
            [DllImport("advapi32", CharSet = CharSet.Unicode, SetLastError = true)] private static extern bool ConvertStringSecurityDescriptorToSecurityDescriptor(string text, uint revision, out IntPtr descriptor, IntPtr length);
            [DllImport("kernel32", CharSet = CharSet.Unicode, SetLastError = true)] private static extern SafePipeHandle CreateNamedPipe(string name, uint open, uint mode, uint instances, uint outSize, uint inSize, uint timeout, ref Attributes security);
            [DllImport("kernel32")] private static extern IntPtr LocalFree(IntPtr value);
            [DllImport("kernel32")] private static extern bool GetNamedPipeClientProcessId(SafePipeHandle pipe, out uint pid);
            [DllImport("kernel32")] public static extern bool PeekNamedPipe(SafePipeHandle pipe, IntPtr buffer, uint size, IntPtr read, IntPtr available, IntPtr remaining);
            [DllImport("kernel32")] private static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
            [DllImport("kernel32", CharSet = CharSet.Unicode)] private static extern bool QueryFullProcessImageName(IntPtr process, uint flags, StringBuilder path, ref uint size);
            [DllImport("kernel32")] private static extern bool CloseHandle(IntPtr handle);
            [DllImport("kernel32")] private static extern IntPtr GetCurrentProcess();
            [DllImport("kernel32")] private static extern IntPtr GetCurrentThread();
            [DllImport("kernel32")] private static extern bool ProcessIdToSessionId(uint pid, out uint session);
            [DllImport("advapi32")] private static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
            [DllImport("advapi32")] private static extern bool OpenThreadToken(IntPtr thread, uint access, bool openAsSelf, out IntPtr token);
            [DllImport("advapi32")] private static extern bool GetTokenInformation(IntPtr token, int kind, IntPtr buffer, uint size, out uint needed);
            [DllImport("advapi32", CharSet = CharSet.Unicode)] private static extern bool ConvertSidToStringSid(IntPtr sid, out IntPtr text);
            [DllImport("advapi32")] private static extern bool ImpersonateNamedPipeClient(SafePipeHandle pipe);
            [DllImport("advapi32")] private static extern bool RevertToSelf();
            private static string TokenSid(IntPtr token)
            {
                GetTokenInformation(token, 1, IntPtr.Zero, 0, out var needed);
                if (needed == 0 || needed > 65536) throw new IOException("Client identity unavailable.");
                var buffer = Marshal.AllocHGlobal((int)needed);
                try
                {
                    if (!GetTokenInformation(token, 1, buffer, needed, out _) || !ConvertSidToStringSid(Marshal.ReadIntPtr(buffer), out var text)) throw new IOException("Client identity unavailable.");
                    try { return Marshal.PtrToStringUni(text); } finally { LocalFree(text); }
                }
                finally { Marshal.FreeHGlobal(buffer); }
            }
            public static string CurrentSid()
            {
                if (!OpenProcessToken(GetCurrentProcess(), 8, out var token)) throw new IOException("User identity unavailable.");
                try { return TokenSid(token); } finally { CloseHandle(token); }
            }
            [DllImport("kernel32")] private static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint pid);
            [DllImport("kernel32", CharSet = CharSet.Unicode)] private static extern bool Process32First(IntPtr snapshot, ref ProcessEntry entry);
            [DllImport("kernel32", CharSet = CharSet.Unicode)] private static extern bool Process32Next(IntPtr snapshot, ref ProcessEntry entry);
            public static NamedPipeServerStream CreatePipe(string name, string sid)
            {
                if (!ConvertStringSecurityDescriptorToSecurityDescriptor("D:P(A;;GA;;;" + sid + ")", 1, out var descriptor, IntPtr.Zero)) throw new IOException("Pipe ACL unavailable.");
                try
                {
                    var attributes = new Attributes { length = Marshal.SizeOf(typeof(Attributes)), descriptor = descriptor };
                    var handle = CreateNamedPipe(@"\\.\pipe\" + name, 0x40000003 | 0x00080000, 0x00000008, 1, MaxBytes, 16384, 3000, ref attributes);
                    if (handle.IsInvalid) { handle.Dispose(); throw new IOException("Unity pipe unavailable."); }
                    return new NamedPipeServerStream(PipeDirection.InOut, true, false, handle);
                }
                finally { LocalFree(descriptor); }
            }
            private static string Image(uint pid)
            {
                var process = OpenProcess(0x1000, false, pid); if (process == IntPtr.Zero) throw new IOException("Client unavailable.");
                try { var path = new StringBuilder(32768); uint size = 32768; if (!QueryFullProcessImageName(process, 0, path, ref size)) throw new IOException("Client path unavailable."); return Path.GetFullPath(path.ToString()); }
                finally { CloseHandle(process); }
            }
            private static uint Parent(uint pid)
            {
                var snapshot = CreateToolhelp32Snapshot(2, 0); if (snapshot == new IntPtr(-1)) throw new IOException("Client unavailable.");
                try { var entry = new ProcessEntry { size = (uint)Marshal.SizeOf(typeof(ProcessEntry)) }; if (Process32First(snapshot, ref entry)) do { if (entry.pid == pid) return entry.parent; } while (Process32Next(snapshot, ref entry)); }
                finally { CloseHandle(snapshot); } throw new IOException("Client unavailable.");
            }
            public static bool Trusted(NamedPipeServerStream pipe, string path, int editor, string sid)
            {
                if (!GetNamedPipeClientProcessId(pipe.SafePipeHandle, out var pid)) return false;
                if (!string.Equals(Image(pid), path, StringComparison.OrdinalIgnoreCase) || !string.Equals(Image(Parent(pid)), path, StringComparison.OrdinalIgnoreCase)) return false;
                if (!ProcessIdToSessionId(pid, out var clientSession) || !ProcessIdToSessionId((uint)editor, out var editorSession) || clientSession != editorSession) return false;
                if (!ImpersonateNamedPipeClient(pipe.SafePipeHandle)) return false;
                try
                {
                    if (!OpenThreadToken(GetCurrentThread(), 8, true, out var token)) return false;
                    try { return TokenSid(token) == sid; } finally { CloseHandle(token); }
                }
                finally { if (!RevertToSelf()) Environment.FailFast("Could not revert local pipe impersonation."); }
            }
        }
    }
}
#endif
