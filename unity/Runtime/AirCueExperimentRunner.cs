using System;
using System.IO;
using System.Linq;
using UnityEngine;

namespace AirCue.Unity
{
    // Explicit next/response calls allow the study to control instructions, breaks and response UI.
    [RequireComponent(typeof(AirCuePlayer))]
    public sealed class AirCueExperimentRunner : MonoBehaviour
    {
        public AirCueExperimentSet experiment;
        public AirCuePlayer.Output output = AirCuePlayer.Output.RoutedQuad;
        public string ParticipantId { get; private set; }
        public string LogPath { get; private set; }
        public ExperimentTrial ActiveTrial { get; private set; }
        public int CompletedTrials { get; private set; }
        public bool CalibrationConfirmed { get; private set; }
        private AirCuePlayer player;
        private AirCueExperimentSet sessionSet;
        private ExperimentTrial[] order;
        private string calibrationConfiguration;
        private double scheduled, observed = -1;
        private long scheduledUnixMs;
        private string revision;
        private bool calibrationPlaying;

        [Serializable] private sealed class Row
        {
            public string sessionId, participantId, trialId, conditionId, stimulusId, revision, projectHash;
            public string eventName, result, abortReason, hrtfMode, outputConfiguration;
            public long unixMs, scheduledUnixMs;
            public double scheduledDspTime, observedDspTime, responseDspTime, reactionTimeMs;
            public float strength = -1, naturalness = -1, comfort = -1;
        }
        private string sessionId;
        private void Awake()
        {
            player = GetComponent<AirCuePlayer>();
            player.playOnStart = false;
        }
        private string Configuration()
        {
            var c = AudioSettings.GetConfiguration();
            return $"{output};{c.speakerMode};{c.sampleRate};{c.dspBufferSize};{(experiment ? experiment.GetInstanceID() : 0)}";
        }
        public void BeginParticipant(string anonymousId)
        {
            if (!player) { player = GetComponent<AirCuePlayer>(); player.playOnStart = false; }
            if (ActiveTrial != null) throw new InvalidOperationException("Abort or answer the active trial first.");
            if (!experiment || !experiment.manifest.plan.participants.Contains(anonymousId)) throw new ArgumentException("Participant ID is not in the manifest.");
            player.Stop();
            sessionSet = experiment;
            ParticipantId = anonymousId;
            order = experiment.manifest.trials.Where(t => t.participantId == anonymousId).ToArray();
            CompletedTrials = 0;
            CalibrationConfirmed = false;
            calibrationPlaying = false;
            sessionId = Guid.NewGuid().ToString("N");
            LogPath = Path.Combine(Application.persistentDataPath, "aircue-" + sessionId + ".jsonl");
            Write("sessionStarted", "", "");
        }
        // 0: headphone left, 1: headphone right, 2: air left, 3: air right.
        public void PlayCalibration(int index)
        {
            if (order == null || ActiveTrial != null || index < 0 || index > 3) throw new InvalidOperationException("Begin a participant and finish any active trial first.");
            if (index >= 2 && output != AirCuePlayer.Output.RoutedQuad) throw new InvalidOperationException("Air calibration requires RoutedQuad.");
            CalibrationConfirmed = false;
            player.sequence = experiment.calibrationSequences[index];
            player.output = output;
            player.Play();
            if (!player.IsScheduled) throw new InvalidOperationException("Calibration playback failed; check output and loaded audio.");
            calibrationPlaying = true;
            Write("calibrationPlayed", index.ToString(), "");
        }
        public void ConfirmCalibration(bool headphoneSides, bool airSides, bool comfortable)
        {
            if (order == null || ActiveTrial != null || !headphoneSides || !airSides || !comfortable) throw new InvalidOperationException("Confirm headphone sides, air sides and comfort for this participant.");
            player.Stop();
            calibrationPlaying = false;
            calibrationConfiguration = Configuration();
            Write("calibrationConfirmed", "headphones/air/comfort", "");
            CalibrationConfirmed = true;
        }
        public void PlayNext()
        {
            if (order == null || !CalibrationConfirmed || calibrationConfiguration != Configuration()) throw new InvalidOperationException("Participant calibration is required for the current output.");
            if (ActiveTrial != null) throw new InvalidOperationException("Answer or abort the current trial before continuing.");
            if (CompletedTrials >= order.Length) return;
            var trial = order[CompletedTrials];
            int index = Array.FindIndex(experiment.manifest.stimuli, s => s.stimulusId == trial.conditionId);
            var condition = Array.Find(experiment.manifest.plan.conditions, c => c.id == trial.conditionId);
            if (condition.modality != "audio" && output != AirCuePlayer.Output.RoutedQuad) throw new InvalidOperationException("Air stimuli require RoutedQuad; headphone-only playback would omit the puff.");
            player.sequence = experiment.sequences[index];
            player.output = output;
            player.Play();
            if (!player.IsScheduled) throw new InvalidOperationException("Trial playback failed; check output and loaded audio.");
            ActiveTrial = trial;
            revision = experiment.manifest.stimuli[index].revision;
            double onsetMs = condition.modality == "audio" ? 200 : condition.modality == "air" ? 200 + condition.onsetDiffMs : 200 + Math.Min(0, condition.onsetDiffMs);
            scheduled = player.ScheduledDspTime + onsetMs / 1000.0;
            scheduledUnixMs = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() + (long)Math.Round((scheduled - AudioSettings.dspTime) * 1000);
            observed = -1;
            try { Write("scheduled", "", ""); }
            catch { player.Stop(); ActiveTrial = null; throw; }
        }
        private void Update()
        {
            if (ActiveTrial == null) return;
            if (calibrationConfiguration != Configuration()) { Abort("outputConfigurationChanged"); CalibrationConfirmed = false; return; }
            if (observed < 0 && AudioSettings.dspTime >= scheduled)
            {
                // Unity frame observation, NOT a physical sensor or audio-callback timestamp.
                observed = AudioSettings.dspTime;
                Write("startedObserved", "", "");
            }
        }
        private void OnEnable() { AudioSettings.OnAudioConfigurationChanged += OutputChanged; }
        private void OutputChanged(bool deviceChanged)
        {
            CalibrationConfirmed = false;
            if (player && (ActiveTrial != null || calibrationPlaying)) Abort("audioConfigurationChanged");
        }
        public void SubmitDirection(string direction) { SubmitResponse(direction, -1, -1, -1); }
        public void SubmitReaction() { SubmitResponse("response", -1, -1, -1); }
        public void SubmitResponse(string result, float strength, float naturalness, float comfort)
        {
            if (ActiveTrial == null || AudioSettings.dspTime < scheduled) throw new InvalidOperationException("No started trial to answer.");
            var plan = experiment.manifest.plan;
            if (plan.primary == "direction" && !new[] { "left", "right", "front", "back", "uncertain" }.Contains(result)) throw new ArgumentException("Choose a direction or uncertain.");
            if (plan.primary == "reactionTime" && result != "response") throw new ArgumentException("Use SubmitReaction or result=response.");
            string[] names = { "strength", "naturalness", "comfort" };
            float[] ratings = { strength, naturalness, comfort };
            for (int i = 0; i < names.Length; i++)
            {
                bool selected = plan.secondary.Contains(names[i]);
                if (float.IsNaN(ratings[i]) || float.IsInfinity(ratings[i]) || (selected ? ratings[i] < 0 || ratings[i] > 10 : ratings[i] != -1)) throw new ArgumentException("Selected ratings require 0–10; unselected ratings require -1.");
            }
            if (observed < 0) { observed = AudioSettings.dspTime; Write("startedObserved", "", ""); }
            Write("result", result, "", strength, naturalness, comfort);
            player.Stop();
            ActiveTrial = null;
            CompletedTrials++;
            if (CompletedTrials == order.Length) Write("sessionFinished", "", "");
        }
        public void Abort(string reason)
        {
            player.Stop();
            calibrationPlaying = false;
            if (ActiveTrial == null) return;
            if (string.IsNullOrWhiteSpace(reason)) reason = "operatorStopped";
            Write("aborted", "", reason);
            ActiveTrial = null;
            CompletedTrials++; // Never silently repeat an aborted trial under the same ID.
        }
        private void Write(string eventName, string result, string abortReason, float strength = -1, float naturalness = -1, float comfort = -1)
        {
            var r = new Row {
                sessionId = sessionId, participantId = ParticipantId, projectHash = sessionSet.manifest.projectHash,
                trialId = ActiveTrial?.trialId ?? "", conditionId = ActiveTrial?.conditionId ?? "", stimulusId = ActiveTrial?.conditionId ?? "", revision = ActiveTrial == null ? "" : revision,
                eventName = eventName, result = result, abortReason = abortReason, unixMs = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
                scheduledUnixMs = ActiveTrial == null ? 0 : scheduledUnixMs, scheduledDspTime = ActiveTrial == null ? -1 : scheduled, observedDspTime = ActiveTrial == null ? -1 : observed,
                responseDspTime = eventName == "result" ? AudioSettings.dspTime : -1, reactionTimeMs = eventName == "result" ? (AudioSettings.dspTime - scheduled) * 1000 : -1,
                hrtfMode = sessionSet.manifest.hrtfMode, outputConfiguration = Configuration(), strength = strength, naturalness = naturalness, comfort = comfort
            };
            try { File.AppendAllText(LogPath, JsonUtility.ToJson(r) + "\n"); }
            catch { player.Stop(); CalibrationConfirmed = false; throw; }
        }
        private void OnDisable()
        {
            AudioSettings.OnAudioConfigurationChanged -= OutputChanged;
            if (player && (ActiveTrial != null || calibrationPlaying)) Abort("runnerDisabled");
            CalibrationConfirmed = false;
        }
    }
}
