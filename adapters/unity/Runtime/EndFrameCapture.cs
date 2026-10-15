using System;
using System.Collections;
using UnityEngine;

namespace Bebekon.SmartComputer
{
    // Created only for a requested Game View frame in Play Mode; no persistent scene object.
    public sealed class EndFrameCapture : MonoBehaviour
    {
        private float expires;
        private void Update() { if (Time.realtimeSinceStartup > expires) Destroy(gameObject); }
        public static void Request(Func<bool> permitted, Action<Texture2D> completed)
        {
            var owner = new GameObject("Smart Computer frame") { hideFlags = HideFlags.HideAndDontSave };
            var reader = owner.AddComponent<EndFrameCapture>();
            reader.expires = Time.realtimeSinceStartup + 5f;
            reader.StartCoroutine(Read(owner, permitted, completed));
        }
        private static IEnumerator Read(GameObject owner, Func<bool> permitted, Action<Texture2D> completed)
        {
            yield return new WaitForEndOfFrame();
            try
            {
                if (permitted()) completed(ScreenCapture.CaptureScreenshotAsTexture());
            }
            finally { Destroy(owner); }
        }
    }
}
