using System;
using UnityEngine;

namespace AirCue.Unity
{
    [Serializable] public sealed class ExperimentCondition
    {
        public string id, modality, audioSide, airSide;
        public int onsetDiffMs;
        public float gainDb;
    }
    [Serializable] public sealed class ExperimentPlan
    {
        public string template, primary;
        public string[] participants, secondary;
        public ExperimentCondition[] conditions;
        public int repetitions, blocks;
        public long seed;
    }
    [Serializable] public sealed class ExperimentTrial
    {
        public string trialId, participantId, conditionId;
        public int block, repetition;
    }
    [Serializable] public sealed class ExperimentStimulus
    {
        public string stimulusId, revision, sequence;
    }
    [Serializable] public sealed class ExperimentManifest
    {
        public int schemaVersion;
        public string generatorVersion, projectHash, hrtfMode;
        public ExperimentPlan plan;
        public ExperimentTrial[] trials;
        public ExperimentStimulus[] stimuli;
    }
    public sealed class AirCueExperimentSet : ScriptableObject
    {
        public ExperimentManifest manifest;
        public AirCueSequence[] sequences;
        public AirCueSequence[] calibrationSequences;
    }
}
