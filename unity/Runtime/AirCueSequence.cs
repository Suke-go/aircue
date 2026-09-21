using System;
using UnityEngine;

namespace AirCue.Unity
{
    [Serializable]
    public class AirCueMarker
    {
        public string id, name, lane;
        public int startFrame, durationFrames;
    }

    [Serializable]
    public class AirCueManifest
    {
        public int schemaVersion, sampleRate, frames;
        public string generatorVersion;
        public int[] routing;
        public AirCueMarker[] markers;
    }

    public sealed class AirCueSequence : ScriptableObject
    {
        public AudioClip headphones;
        public AudioClip routedFourChannel;
        public AirCueManifest manifest;
    }
}
