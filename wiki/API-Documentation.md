# API Documentation

## Remote transcription

Both clients use:

```text
POST {base_url}/audio/transcriptions
```

Multipart fields:

| Field | Required | Description |
|---|---:|---|
| `file` | Yes | WAV audio, normally 16 kHz mono |
| `model` | No | Remote model id; omitted for automatic selection |
| `language` | No | `ru`, `en`, `zh`; omitted for automatic detection |
| `response_format` | Yes | `json` |

Expected response:

```json
{"text":"recognized text"}
```

The API key is sent as `Authorization: Bearer ...` when configured. Rust Settings never returns the raw key to the frontend.

## Model discovery

```text
GET {base_url}/models
```

Rust Settings uses this to populate the remote model picker. The picker is refreshed on launch and can be refreshed manually; providers without `/models` can still be used by choosing **Custom model id** and entering a model id manually.

## Provider templates

| Provider | Default endpoint |
|---|---|
| OpenAI | `https://api.openai.com/v1` |
| OpenRouter | `https://openrouter.ai/api/v1` |
| Custom | User supplied |

## Local command provider

The Rust local provider accepts a template such as:

```text
parakeet-asr --in {input_path} --out {output_path} --lang {language}
```

On Unix it runs through `sh -c`; on Windows through `cmd /C`. Successful output may be plain text in the output file or stdout. Empty output and non-zero exit status become visible transcription errors.

## Warm-up request

When **Warm up remote server at speech start** is enabled, the client sends a short silence WAV to the transcription endpoint before real audio. This is best-effort and is intended for a sleeping/self-hosted Whisper service.

## Proxy behavior

The Rust HTTP client retains reqwest's system/environment proxy behavior. Common variables are:

```text
HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY
```

The Swift client uses the platform URL loading stack and preserves its existing environment overrides. Never commit proxy credentials or API keys.

## Error contract

Errors should identify the class of failure: invalid endpoint, missing key, HTTP status/body, missing local model, local command failure, empty output, or denied permission. The overlay uses an icon and tooltip; the record window can show the detailed text.
