using System;
using System.IO;
using System.Linq;
using AirCue.Unity;
using UnityEditor;
using UnityEngine;

public static class AirCueExperimentVerification
{
    public static void Run()
    {
        const string path = "Assets/AirCueUnity/Experiment.aircueexp";
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
        Require(prefab, "experiment prefab imported");
        var runner = prefab.GetComponent<AirCueExperimentRunner>();
        Require(runner && runner.output == AirCuePlayer.Output.RoutedQuad, "quad experiment default");
        Require(!prefab.GetComponent<AirCuePlayer>().playOnStart, "no automatic stimulus");
        var set = runner.experiment;
        Require(set && set.sequences.Length == 6 && set.calibrationSequences.Length == 4, "stimuli and calibration references");
        Require(set.manifest.plan.template == "onset" && set.manifest.trials.Length == 24, "manifest order");
        for (int index = 0; index < 6; index++)
        {
            var sequence = set.sequences[index];
            var condition = set.manifest.plan.conditions[index];
            var audio = sequence.manifest.markers.Single(m => m.lane == "audio");
            var air = sequence.manifest.markers.Single(m => m.lane != "audio");
            Require(audio.startFrame == 9600 && air.startFrame - audio.startFrame == condition.onsetDiffMs * 48, "signed baked onset difference");
            var quad = new float[sequence.routedFourChannel.samples * 4];
            var stereo = new float[sequence.headphones.samples * 2];
            Require(sequence.routedFourChannel.GetData(quad, 0) && sequence.headphones.GetData(stereo, 0), "PCM load");
            for (int i = 0; i < stereo.Length / 2; i++)
            {
                Require(Math.Abs(stereo[i * 2] - quad[i * 4 + 2]) < 0.000001f, "baked left equality");
                Require(Math.Abs(stereo[i * 2 + 1] - quad[i * 4 + 3]) < 0.000001f, "baked right equality");
            }
        }
        int[] channels = { 2, 3, 1, 0 };
        for (int index = 0; index < 4; index++)
        {
            var clip = set.calibrationSequences[index].routedFourChannel;
            var data = new float[clip.samples * 4];
            Require(clip.GetData(data, 0), "calibration PCM");
            Require(data.Where((v, i) => i % 4 == channels[index]).Any(v => v != 0), "calibration selected channel");
            Require(data.Where((v, i) => i % 4 != channels[index]).All(v => v == 0), "calibration other channels silent");
        }
        AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceUpdate);
        Require(AssetDatabase.LoadAssetAtPath<GameObject>(path).GetComponent<AirCueExperimentRunner>().experiment.sequences.All(s => s), "reimport references");
        var instance = UnityEngine.Object.Instantiate(prefab);
        var live = instance.GetComponent<AirCueExperimentRunner>();
        live.BeginParticipant("P001");
        Require(!live.CalibrationConfirmed && live.CompletedTrials == 0, "new participant calibration reset");
        bool blocked = false;
        try { live.PlayNext(); } catch (InvalidOperationException) { blocked = true; }
        Require(blocked, "uncalibrated trial blocked before playback");
        live.ConfirmCalibration(true, true, true);
        Require(live.CalibrationConfirmed, "confirmation recorded");
        instance.SendMessage("OutputChanged", true);
        Require(!live.CalibrationConfirmed, "device changes invalidate calibration");
        string log = File.ReadAllText(live.LogPath);
        Require(log.Contains("calibrationConfirmed") && log.Contains(set.manifest.projectHash) && log.Contains("P001"), "session log provenance");
        UnityEngine.Object.DestroyImmediate(instance);
        Debug.Log("AIRCUE_EXPERIMENT_VERIFIED: manifest, prefab, timing, PCM, calibration routing, reimport, calibration gate and log");
    }
    private static void Require(bool condition, string description)
    {
        if (!condition) throw new Exception("AirCue experiment verification failed: " + description);
    }
}
