using System;
using System.Collections.Concurrent;
using System.IO;
using System.Net.Sockets;
using System.Text;
using System.Threading.Tasks;
using UnityEngine;
using UnityEngine.Events;

namespace AirCue.Unity
{
    [DisallowMultipleComponent]
    public sealed class AirCueExperimentClient : MonoBehaviour
    {
        public string host = "127.0.0.1";
        [Range(1024, 65535)] public int port = 39100;
        [Tooltip("AirCueの実験連携画面に表示される起動ごとのトークン")]
        public string sessionToken;
        public string trialId = "trial-001";
        public enum Target { All, Audio, Air }
        public Target target = Target.All;
        [Range(50, 5000)] public int delayMs = 100;
        public UnityEvent<string> responseReceived;
        public UnityEvent<string> errorReceived;
        private readonly ConcurrentQueue<Action> callbacks = new ConcurrentQueue<Action>();

        public void CheckStatus() { Send("status", null, null); }
        public void Prepare() { Send("prepare", null, TargetName()); }
        public void Play() { Send("play", trialId, null); }
        public void StopRemote() { Send("stop", trialId, null); }

        private string TargetName()
        {
            return target == Target.Audio ? "audio" : target == Target.Air ? "air" : "all";
        }

        private void Send(string command, string trial, string requestedTarget)
        {
            if (host != "127.0.0.1" && host != "localhost")
            { errorReceived?.Invoke("AirCueは同じPCの127.0.0.1だけを使用します。"); return; }
            if (string.IsNullOrWhiteSpace(sessionToken))
            { errorReceived?.Invoke("AirCueのセッショントークンを入力してください。"); return; }
            string requestId = Guid.NewGuid().ToString("N");
            var request = new Request {
                command = command, requestId = requestId, token = sessionToken,
                trialId = trial, cueId = command == "prepare" ? "current" : null,
                target = requestedTarget, delayMs = delayMs
            };
            Task.Run(() => Exchange(JsonUtility.ToJson(request)))
                .ContinueWith(task => callbacks.Enqueue(() => {
                    if (task.IsFaulted) errorReceived?.Invoke(task.Exception.GetBaseException().Message);
                    else responseReceived?.Invoke(task.Result);
                }));
        }

        private string Exchange(string json)
        {
            using (var client = new TcpClient())
            {
                var connection = client.ConnectAsync(host, port);
                if (!connection.Wait(3000)) throw new TimeoutException("AirCueへの接続がタイムアウトしました。");
                client.ReceiveTimeout = 5000;
                client.SendTimeout = 5000;
                using (NetworkStream stream = client.GetStream())
                using (var writer = new StreamWriter(stream, new UTF8Encoding(false), 1024, true) { AutoFlush = true })
                using (var reader = new StreamReader(stream, Encoding.UTF8, false, 1024, true))
                {
                    writer.WriteLine(json);
                    string line = reader.ReadLine();
                    if (string.IsNullOrEmpty(line)) throw new IOException("AirCueから応答がありません。");
                    return line;
                }
            }
        }

        private void Update()
        {
            while (callbacks.TryDequeue(out Action callback)) callback();
        }

        [Serializable]
        private class Request
        {
            public string command, requestId, token, trialId, cueId, target;
            public int delayMs;
        }
    }
}
