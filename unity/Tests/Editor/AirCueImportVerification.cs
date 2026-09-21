using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;
using AirCue.Unity;

public static class AirCueImportVerification
{
    // Run in an isolated Unity project populated from tests/unity-project.json.
    public static void Run()
    {
        string[] paths = Directory.GetFiles("Assets/AirCueUnity/Sequences", "Sequence.aircueseq", SearchOption.AllDirectories);
        Require(paths.Length == 1, "one exported sequence");
        string path = paths[0].Replace('\\', '/');
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
        Require(prefab, "imported prefab");
        var player = prefab.GetComponent<AirCuePlayer>();
        Require(player && player.output == AirCuePlayer.Output.Headphones, "headphone default");
        var sequence = player.sequence;
        Require(sequence && sequence.manifest.schemaVersion == 1, "sequence subasset");
        Require(sequence.manifest.frames == 9600, "frame count");
        Require(sequence.manifest.routing.SequenceEqual(new[] { 3, 4, 2, 1 }), "routing permutation");
        Require(sequence.manifest.markers[0].startFrame == 960, "audio marker");
        Require(sequence.manifest.markers[1].startFrame == 4800, "air marker");
        var stereo = new float[9600 * 2]; var quad = new float[9600 * 4];
        Require(sequence.headphones.GetData(stereo, 0), "stereo PCM loaded");
        Require(sequence.routedFourChannel.GetData(quad, 0), "quad PCM loaded");
        for (int i = 0; i < 9600; i++)
        {
            Require(Math.Abs(stereo[i * 2] - quad[i * 4 + 2]) < 0.000001f, "left headphone routing");
            Require(Math.Abs(stereo[i * 2 + 1] - quad[i * 4 + 3]) < 0.000001f, "right headphone routing");
            Require(quad[i * 4] == 0, "unused air-right channel silent");
            if (i < 4800) Require(quad[i * 4 + 1] == 0, "air onset preserved");
        }
        Require(quad.Where((v, i) => i % 4 == 1).Any(v => v != 0), "air pulse present");
        var importer = (AudioImporter)AssetImporter.GetAtPath(Path.GetDirectoryName(path).Replace('\\', '/') + "/headphones.wav");
        Require(!importer.forceToMono && importer.defaultSampleSettings.compressionFormat == AudioCompressionFormat.PCM, "lossless import settings");
        AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceUpdate);
        Require(AssetDatabase.LoadAssetAtPath<GameObject>(path).GetComponent<AirCuePlayer>().sequence, "reimport retains references");
        Debug.Log("AIRCUE_UNITY_VERIFIED: prefab, references, PCM channels, frame timing, routing and reimport");
    }
    private static void Require(bool condition, string description)
    {
        if (!condition) throw new Exception("AirCue verification failed: " + description);
    }
}
