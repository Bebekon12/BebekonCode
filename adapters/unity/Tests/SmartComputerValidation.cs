#if UNITY_EDITOR_WIN
using System;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using Bebekon.SmartComputer.Editor;
public static class SmartComputerValidation {
 public static void Run() {
  EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
  var go=new GameObject("Original");
  var command=new DesktopAdapter.Command{generation=DesktopAdapter.Generation,kind="rename",object_id=DesktopAdapter.Reference(go),name="Changed"};
  if(!DesktopAdapter.Apply(command)||go.name!="Changed")throw new Exception("rename verification failed");
  Undo.FlushUndoRecordObjects();Undo.PerformUndo();if(go.name!="Original")throw new Exception("rename undo failed");
  command=new DesktopAdapter.Command{generation=DesktopAdapter.Generation,kind="set_active",object_id=DesktopAdapter.Reference(go),active=false};
  if(!DesktopAdapter.Apply(command)||go.activeSelf)throw new Exception("active verification failed");
  Undo.FlushUndoRecordObjects();Undo.PerformUndo();if(!go.activeSelf)throw new Exception("active undo failed");
  command=new DesktopAdapter.Command{generation=DesktopAdapter.Generation,kind="set_transform",object_id=DesktopAdapter.Reference(go),position=new[]{1f,2f,3f}};
  if(!DesktopAdapter.Apply(command)||go.transform.localPosition!=new Vector3(1,2,3))throw new Exception("transform verification failed");
  Undo.FlushUndoRecordObjects();Undo.PerformUndo();if(go.transform.localPosition!=Vector3.zero)throw new Exception("transform undo failed");
  bool refused=false;try{DesktopAdapter.Apply(new DesktopAdapter.Command{generation=DesktopAdapter.Generation,kind="eval",object_id=DesktopAdapter.Reference(go)});}catch(InvalidOperationException){refused=true;}
  if(!refused)throw new Exception("arbitrary code was accepted");
  Debug.Log("SMART_COMPUTER_VALIDATION_PASSED: rename, active, transform, Undo, eval refusal");
  UnityEngine.Object.DestroyImmediate(go);
 }
}

#endif
