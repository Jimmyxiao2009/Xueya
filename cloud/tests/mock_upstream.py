#!/usr/bin/env python3
"""A tiny OpenAI-compatible upstream used by the Docker Compose integration env.

It answers the two endpoints the Verruckt Cloud AI gateway calls:
  POST /v1/chat/completions
  POST /v1/embeddings

The chat endpoint supports both JSON and SSE (`stream: true`), and both include
`usage` so the cloud quota path can be exercised without a real provider.
"""

import json
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def embedding(text: str, dimensions: int = 8):
    vector = [0.0] * dimensions
    for index, char in enumerate(text):
        vector[index % dimensions] += (ord(char) % 17) / 17.0
    return vector


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        # Keep the compose logs readable.
        return

    def _read_json(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b"{}"
        return json.loads(raw.decode("utf-8") or "{}")

    def _send_json(self, status, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path in ("/healthz", "/health"):
            self._send_json(200, {"status": "ok"})
            return
        if self.path.endswith("/models"):
            self._send_json(200, {"object": "list", "data": []})
            return
        self._send_json(404, {"error": {"message": "not found"}})

    def do_POST(self):
        body = self._read_json()
        if self.path.endswith("/chat/completions"):
            self.chat(body)
            return
        if self.path.endswith("/embeddings"):
            self.embeddings(body)
            return
        self._send_json(404, {"error": {"message": "not found"}})

    def chat(self, body):
        model = body.get("model", "mock")
        messages = body.get("messages") or []
        prompt = ""
        for message in messages:
            if message.get("role") == "user":
                prompt = str(message.get("content", ""))[:80]
        answer = f"mock upstream reply for: {prompt or 'verruckt'}"
        usage = {
            "prompt_tokens": max(1, len(prompt) // 4),
            "completion_tokens": max(1, len(answer) // 4),
            "total_tokens": max(2, (len(prompt) + len(answer)) // 4),
        }

        if body.get("stream"):
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Connection", "keep-alive")
            self.end_headers()
            pieces = [answer[: len(answer) // 2], answer[len(answer) // 2 :]]
            for index, piece in enumerate(pieces):
                chunk = {
                    "id": "chatcmpl-mock",
                    "object": "chat.completion.chunk",
                    "created": int(time.time()),
                    "model": model,
                    "choices": [
                        {
                            "index": 0,
                            "delta": {"content": piece},
                            "finish_reason": None,
                        }
                    ],
                }
                self.wfile.write(f"data: {json.dumps(chunk)}\n\n".encode("utf-8"))
                self.wfile.flush()
            final = {
                "id": "chatcmpl-mock",
                "object": "chat.completion.chunk",
                "created": int(time.time()),
                "model": model,
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                "usage": usage,
            }
            self.wfile.write(f"data: {json.dumps(final)}\n\n".encode("utf-8"))
            self.wfile.write(b"data: [DONE]\n\n")
            self.wfile.flush()
            return

        self._send_json(
            200,
            {
                "id": "chatcmpl-mock",
                "object": "chat.completion",
                "created": int(time.time()),
                "model": model,
                "choices": [
                    {
                        "index": 0,
                        "message": {"role": "assistant", "content": answer},
                        "finish_reason": "stop",
                    }
                ],
                "usage": usage,
            },
        )

    def embeddings(self, body):
        raw = body.get("input", "")
        inputs = [raw] if isinstance(raw, str) else list(raw)
        data = []
        for index, text in enumerate(inputs):
            data.append(
                {
                    "object": "embedding",
                    "index": index,
                    "embedding": embedding(str(text)),
                }
            )
        self._send_json(
            200,
            {
                "object": "list",
                "data": data,
                "model": body.get("model", "mock-embedding"),
                "usage": {
                    "prompt_tokens": max(1, sum(len(str(item)) for item in inputs) // 4),
                    "total_tokens": max(1, sum(len(str(item)) for item in inputs) // 4),
                },
            },
        )


if __name__ == "__main__":
    server = ThreadingHTTPServer(("0.0.0.0", 8099), Handler)
    server.serve_forever()
