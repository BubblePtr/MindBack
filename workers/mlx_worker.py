#!/usr/bin/env python3
"""MindBack MLX-VLM worker contract.

The worker always prints one JSON object. If mlx-vlm is not installed or the
image cannot be read, it returns a structured error instead of a traceback.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def emit(payload: dict[str, Any]) -> None:
    print(json.dumps(payload, ensure_ascii=False))


def unavailable(reason: str) -> int:
    emit(
        {
            "error": "worker_unavailable",
            "intent": "未能完成本地视觉识别",
            "is_on_project": False,
            "confidence": 0.0,
            "reason": reason,
            "visible_context": "",
        }
    )
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run MindBack MLX-VLM recognition")
    parser.add_argument("--model", required=True)
    parser.add_argument("--project", required=True)
    parser.add_argument("--image", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    image_path = Path(args.image).expanduser()
    if not image_path.exists():
        return unavailable(f"image not found: {image_path}")

    try:
        from mlx_vlm import generate, load  # type: ignore
    except Exception as exc:  # pragma: no cover - depends on local MLX install
        return unavailable(f"mlx-vlm is not available: {exc}")

    prompt = (
        "你是 MindBack 的本地监督日志识别器。"
        "请判断截图中的当前行为是否符合今日项目，并只输出 JSON。"
        f"今日项目：{args.project}\n"
        "字段必须包含 intent, is_on_project, confidence, reason, visible_context。"
    )

    try:
        model, processor = load(args.model)
        response = generate(
            model,
            processor,
            prompt=prompt,
            image=str(image_path),
            max_tokens=256,
            temperature=0.0,
        )
    except Exception as exc:  # pragma: no cover - depends on local MLX/model state
        return unavailable(f"mlx-vlm recognition failed: {exc}")

    text = str(response).strip()
    try:
        payload = json.loads(text)
    except json.JSONDecodeError:
        emit(
            {
                "error": "parse_error",
                "intent": "模型返回了非 JSON 内容",
                "is_on_project": False,
                "confidence": 0.0,
                "reason": text[:500],
                "visible_context": "",
            }
        )
        return 0

    payload.setdefault("error", None)
    emit(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
