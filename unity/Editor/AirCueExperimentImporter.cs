using System;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using UnityEditor;
using UnityEditor.AssetImporters;
using UnityEngine;

namespace AirCue.Unity.Editor
{
    [ScriptedImporter(1, "aircueexp", 200)]
    public sealed class AirCueExperimentImporter : ScriptedImporter
    {
        private static ExperimentManifest Read(string path)
        {
            var m = JsonUtility.FromJson<ExperimentManifest>(File.ReadAllText(path));
            if (m == null || m.schemaVersion != 1 || m.plan == null || m.plan.conditions == null || m.stimuli == null || m.trials == null || m.plan.participants == null || m.plan.secondary == null || m.stimuli.Length == 0 || m.stimuli.Length > 32 || !Regex.IsMatch(m.projectHash ?? "", "^[a-f0-9]{64}$")) throw new InvalidDataException("Invalid experiment manifest.");
            if (m.stimuli.Select(s => s.stimulusId).Distinct().Count() != m.stimuli.Length || m.plan.conditions.Length != m.stimuli.Length || m.trials.Select(t => t.trialId).Distinct().Count() != m.trials.Length) throw new InvalidDataException("Duplicate or missing conditions/trials.");
            foreach (var s in m.stimuli)
                if (!Regex.IsMatch(s.stimulusId ?? "", "^[A-Za-z0-9_-]{1,40}$") || s.sequence != $"Sequences/Sequence-{s.stimulusId}/Sequence.aircueseq" || !Regex.IsMatch(s.revision ?? "", "^[a-f0-9]{64}$") || m.plan.conditions.Count(c => c.id == s.stimulusId) != 1) throw new InvalidDataException("Invalid stimulus path or condition.");
            foreach (var t in m.trials)
                if (!m.stimuli.Any(s => s.stimulusId == t.conditionId) || !m.plan.participants.Contains(t.participantId)) throw new InvalidDataException("Unknown trial binding.");
            return m;
        }
        public static string[] GatherDependenciesFromSourceFile(string path)
        {
            var m = Read(path);
            string folder = Path.GetDirectoryName(path).Replace('\\', '/');
            return m.stimuli.Select(s => folder + "/" + s.sequence).Concat(Enumerable.Range(0, 4).Select(i => folder + $"/Sequences/Sequence-calibration-{i}/Sequence.aircueseq")).ToArray();
        }
        public override void OnImportAsset(AssetImportContext ctx)
        {
            var m = Read(ctx.assetPath);
            string[] paths = GatherDependenciesFromSourceFile(ctx.assetPath);
            var sequences = paths.Select(path => {
                ctx.DependsOnArtifact(path);
                var seq = AssetDatabase.LoadAllAssetsAtPath(path).OfType<AirCueSequence>().FirstOrDefault();
                if (!seq) throw new InvalidDataException("Missing baked sequence: " + path);
                return seq;
            }).ToArray();
            var set = ScriptableObject.CreateInstance<AirCueExperimentSet>();
            set.manifest = m;
            set.sequences = sequences.Take(m.stimuli.Length).ToArray();
            set.calibrationSequences = sequences.Skip(m.stimuli.Length).ToArray();
            ctx.AddObjectToAsset("experiment", set);
            var root = new GameObject("AirCue Experiment");
            root.AddComponent<AudioSource>().playOnAwake = false;
            root.AddComponent<AirCuePlayer>().playOnStart = false;
            root.AddComponent<AirCueExperimentRunner>().experiment = set;
            ctx.AddObjectToAsset("runner", root);
            ctx.SetMainObject(root);
        }
    }
}
