using System;
using System.IO;
using UnityEditor;
using UnityEditor.AssetImporters;
using UnityEngine;

namespace AirCue.Unity.Editor
{
    [ScriptedImporter(1, "aircueseq", 100)]
    public sealed class AirCueImporter : ScriptedImporter
    {
        public static string[] GatherDependenciesFromSourceFile(string path)
        {
            string folder = Path.GetDirectoryName(path).Replace('\\', '/');
            return new[] { folder + "/headphones.wav", folder + "/routed-4ch.wav" };
        }

        public override void OnImportAsset(AssetImportContext ctx)
        {
            AirCueManifest data = JsonUtility.FromJson<AirCueManifest>(File.ReadAllText(ctx.assetPath));
            if (data == null || data.schemaVersion != 1 || data.sampleRate != 48000 || data.frames <= 0 || data.frames > 48000 * 60)
                throw new InvalidDataException("Unsupported AirCue sequence metadata.");
            if (data.routing == null || data.routing.Length != 4)
                throw new InvalidDataException("AirCue routing must contain four channels.");
            var used = new bool[4];
            foreach (int ch in data.routing)
            {
                if (ch < 1 || ch > 4 || used[ch - 1]) throw new InvalidDataException("Invalid AirCue routing.");
                used[ch - 1] = true;
            }
            string[] paths = GatherDependenciesFromSourceFile(ctx.assetPath);
            foreach (string path in paths) ctx.DependsOnArtifact(path);
            var stereo = AssetDatabase.LoadAssetAtPath<AudioClip>(paths[0]);
            var quad = AssetDatabase.LoadAssetAtPath<AudioClip>(paths[1]);
            ValidateAudio(stereo, 2, data);
            ValidateAudio(quad, 4, data);
            var sequence = ScriptableObject.CreateInstance<AirCueSequence>();
            sequence.name = "Sequence";
            sequence.headphones = stereo;
            sequence.routedFourChannel = quad;
            sequence.manifest = data;
            ctx.AddObjectToAsset("sequence", sequence);
            var root = new GameObject(Path.GetFileNameWithoutExtension(ctx.assetPath));
            root.AddComponent<AudioSource>().playOnAwake = false;
            root.AddComponent<AirCuePlayer>().sequence = sequence;
            ctx.AddObjectToAsset("player", root);
            ctx.SetMainObject(root);
        }

        private static void ValidateAudio(AudioClip clip, int channels, AirCueManifest data)
        {
            if (!clip || clip.channels != channels || clip.frequency != data.sampleRate || clip.samples != data.frames)
                throw new InvalidDataException("AirCue audio is missing or has different channels, sample rate or length. Copy the complete export folder.");
        }
    }

    // Only files next to an AirCue sequence get these settings.
    public sealed class AirCueAudioImportSettings : AssetPostprocessor
    {
        private void OnPreprocessAudio()
        {
            string folder = Path.GetDirectoryName(assetPath);
            if (!File.Exists(Path.Combine(folder, "Sequence.aircueseq"))) return;
            string name = Path.GetFileName(assetPath);
            if (name != "headphones.wav" && name != "routed-4ch.wav") return;
            var importer = (AudioImporter)assetImporter;
            importer.forceToMono = false;
            importer.ambisonic = false;
            importer.loadInBackground = false;
            var settings = importer.defaultSampleSettings;
            settings.compressionFormat = AudioCompressionFormat.PCM;
            settings.loadType = AudioClipLoadType.DecompressOnLoad;
            settings.sampleRateSetting = AudioSampleRateSetting.PreserveSampleRate;
            settings.preloadAudioData = true;
            importer.defaultSampleSettings = settings;
        }
    }
}
