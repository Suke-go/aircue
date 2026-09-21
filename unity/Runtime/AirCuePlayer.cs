using System;
using UnityEngine;

namespace AirCue.Unity
{
    [RequireComponent(typeof(AudioSource))]
    [DisallowMultipleComponent]
    public sealed class AirCuePlayer : MonoBehaviour
    {
        public enum Output { Headphones, RoutedQuad }
        public AirCueSequence sequence;
        public Output output = Output.Headphones;
        public bool playOnStart = true;
        [Min(0.05f)] public float leadTime = 0.1f;
        public double ScheduledDspTime { get; private set; }
        public bool IsScheduled { get; private set; }
        public event Action<double> Scheduled;
        private AudioSource source;

        private void Awake()
        {
            source = GetComponent<AudioSource>();
            source.playOnAwake = false;
        }
        private void Start() { if (playOnStart) Play(); }

        // Callable by a UnityEvent, UI Button, or gameplay script.
        public void Play()
        {
            if (!sequence) { Debug.LogError("AirCue sequence is missing.", this); return; }
            if (!source) source = GetComponent<AudioSource>();
            Stop();
            bool quad = output == Output.RoutedQuad;
            if (quad && AudioSettings.GetConfiguration().speakerMode != AudioSpeakerMode.Quad)
            {
                Debug.LogError("AirCue: RoutedQuad requires a verified four-channel device and Unity Audio Speaker Mode Quad. Stereo fallback is disabled.", this);
                return;
            }
            AudioClip clip = quad ? sequence.routedFourChannel : sequence.headphones;
            if (!clip || clip.channels != (quad ? 4 : 2))
            { Debug.LogError("AirCue: invalid or missing audio clip.", this); return; }
            if (clip.loadState != AudioDataLoadState.Loaded)
            { clip.LoadAudioData(); Debug.LogWarning("AirCue: audio is loading. Call Play again after loading.", this); return; }
            source.clip = clip;
            source.spatialBlend = 0;
            source.spatialize = false;
            source.spatializePostEffects = false;
            source.panStereo = 0;
            source.pitch = 1;
            source.volume = 1;
            source.loop = false;
            source.dopplerLevel = 0;
            source.bypassEffects = true;
            source.bypassListenerEffects = true;
            source.bypassReverbZones = true;
            source.outputAudioMixerGroup = null;
            ScheduledDspTime = AudioSettings.dspTime + Math.Max(0.05, leadTime);
            source.PlayScheduled(ScheduledDspTime);
            IsScheduled = true;
            Scheduled?.Invoke(ScheduledDspTime);
        }

        public void Stop()
        {
            if (source) source.Stop();
            IsScheduled = false;
        }
        private void Update()
        {
            if (IsScheduled && sequence && AudioSettings.dspTime >= ScheduledDspTime + (double)sequence.manifest.frames / sequence.manifest.sampleRate)
                IsScheduled = false;
        }
        private void OnDisable() { Stop(); }
    }
}
