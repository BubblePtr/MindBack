#!/usr/bin/env python3
"""MindBack MLX-VLM worker.

Supports two modes:
  1. One-shot (default): loads model, runs one inference, exits.
  2. Daemon (--daemon): loads model once, listens on stdin for JSON requests,
     writes JSON responses to stdout.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


def emit(payload: dict[str, Any]) -> None:
    print(json.dumps(payload, ensure_ascii=False), flush=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="MindBack MLX-VLM worker")
    parser.add_argument("--model", required=True)
    parser.add_argument("--project", default="")
    parser.add_argument("--image", default="")
    parser.add_argument("--daemon", action="store_true", help="Run in daemon mode")
    return parser.parse_args()


def build_prompt(project: str) -> str:
    project_text = project if project.strip() else "今日项目"
    return (
        "你是一个屏幕内容观察者。请观察截图，描述用户正在做什么，并判断是否与其当前的工作项目相关。"
        "你只能看到截图里实际显示的内容，不能猜测、联想或推断未直接可见的信息。\n\n"
        "## 输出格式\n"
        "严格 JSON，字段：intent, is_on_project, confidence, reason, visible_context\n\n"
        "## 规则\n"
        "1. intent（≤30字）：一句话概括用户在做什么。禁止编造未看到的内容。\n"
        "2. is_on_project：仅基于截图中明确可见的证据判断。不确定时优先 false。\n"
        "3. confidence：0~1。≥0.8 仅用于应用名和内容都清晰且与项目明确相关。"
        "0.4~0.7 用于部分证据。<0.4 用于看不清、不确定。\n"
        "4. reason（≤60字）：说明判断依据，必须提到实际看到的应用名或窗口标题。\n"
        "5. visible_context（≤40字）：记录屏幕上可见的关键证据（应用名、窗口标题、文件名）。"
        "禁止复制大段文本。\n"
        "6. 所有值使用简体中文，代码/文件名/专有名词除外。\n"
        "7. 如果截图模糊、黑暗或文字不可读，confidence 必须 <0.4，"
        'intent 写"无法识别当前内容"。\n'
        "8. 不要猜测未显示的内容、未打开的窗口、未访问的页面。\n"
        "9. 如果左上角菜单栏应用名不可读，不要编造应用名。\n"
        "10. 如果截图显示的是监督、复盘或记录类应用的界面，"
        '只用"正在查看系统记录界面"概括，不要复制界面上的日志或数据内容。\n'
        "11. 如果左上角菜单栏显示的应用名与你正在观察的系统本身相同，"
        "不要因此认定用户在用那个系统——仔细看截图中实际的窗口内容。\n\n"
        "## 示例\n"
        '截图：左上角"Cursor"，编辑器打开 recognition.rs → '
        '{"intent":"在 Cursor 中编辑 Rust 代码","is_on_project":true,"confidence":0.92,'
        '"reason":"左上角活跃应用是 Cursor，编辑器打开一个 Rust 项目的 recognition.rs。",'
        '"visible_context":"Cursor；recognition.rs"}\n'
        '截图：屏幕很暗，看不清内容 → '
        '{"intent":"无法识别当前内容","is_on_project":false,"confidence":0.15,'
        '"reason":"截图过暗，无法识别任何可见内容。","visible_context":"屏幕过暗"}\n\n'
        f"今日项目：{project_text}\n"
        "现在请观察截图并输出 JSON："
    )


def parse_model_json(text: str) -> dict[str, Any]:
    text = text.strip()
    if text.startswith("```"):
        text = re.sub(r"^```(?:json)?\s*", "", text)
        text = re.sub(r"\s*```$", "", text)

    try:
        return json.loads(text)
    except json.JSONDecodeError:
        decoder = json.JSONDecoder()
        match = re.search(r"\{", text)
        if match:
            payload, _ = decoder.raw_decode(text[match.start() :])
            if isinstance(payload, dict):
                return payload
        raise


def normalize_payload(payload: dict[str, Any]) -> dict[str, Any]:
    def stringify(value: Any) -> str:
        if value is None:
            return ""
        if isinstance(value, str):
            return value.strip()
        if isinstance(value, list):
            return "；".join(stringify(item) for item in value if stringify(item))
        if isinstance(value, dict):
            return json.dumps(value, ensure_ascii=False)
        return str(value).strip()

    def bool_field(value: Any) -> bool:
        if isinstance(value, bool):
            return value
        if isinstance(value, str):
            return value.strip().lower() in {"true", "yes", "1", "是", "符合"}
        return bool(value)

    def confidence_field(value: Any) -> float:
        try:
            return max(0.0, min(1.0, float(value)))
        except (TypeError, ValueError):
            return 0.0

    return {
        "intent": stringify(payload.get("intent")) or "未能判断当前行为",
        "is_on_project": bool_field(payload.get("is_on_project")),
        "confidence": confidence_field(payload.get("confidence")),
        "reason": stringify(payload.get("reason")),
        "visible_context": stringify(payload.get("visible_context")),
        "error": payload.get("error"),
    }


def make_error_response(error: str) -> dict[str, Any]:
    return {
        "error": error,
        "intent": "未能完成本地视觉识别",
        "is_on_project": False,
        "confidence": 0.0,
        "reason": error,
        "visible_context": "",
    }


def run_once(args: argparse.Namespace) -> int:
    image_path = Path(args.image).expanduser()
    if not image_path.exists():
        emit(make_error_response(f"图片不存在：{image_path}"))
        return 0

    try:
        from mlx_vlm import apply_chat_template, generate, load  # type: ignore
    except Exception as exc:  # pragma: no cover
        emit(make_error_response(f"mlx-vlm 不可用：{exc}"))
        return 0

    try:
        model, processor = load(args.model)
    except Exception as exc:  # pragma: no cover
        emit(make_error_response(f"模型加载失败：{exc}"))
        return 0

    prompt = build_prompt(args.project)
    try:
        formatted_prompt = apply_chat_template(
            processor, model.config, prompt, num_images=1
        )
        response = generate(
            model,
            processor,
            prompt=formatted_prompt,
            image=[str(image_path)],
            max_tokens=256,
            temperature=0.0,
        )
    except Exception as exc:  # pragma: no cover
        emit(make_error_response(f"识别失败：{exc}"))
        return 0

    text = getattr(response, "text", str(response)).strip()
    try:
        payload = parse_model_json(text)
    except json.JSONDecodeError:
        emit({
            "error": "parse_error",
            "intent": "模型返回了非 JSON 内容",
            "is_on_project": False,
            "confidence": 0.0,
            "reason": text[:500],
            "visible_context": "",
        })
        return 0

    emit(normalize_payload(payload))
    return 0


def run_daemon(args: argparse.Namespace) -> int:
    try:
        from mlx_vlm import apply_chat_template, generate, load  # type: ignore
    except Exception as exc:  # pragma: no cover
        emit({"__ready__": False, "error": f"mlx-vlm 不可用：{exc}"})
        return 1

    try:
        model, processor = load(args.model)
    except Exception as exc:  # pragma: no cover
        emit({"__ready__": False, "error": f"模型加载失败：{exc}"})
        return 1

    emit({"__ready__": True})

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError:
            emit(make_error_response("无效的 JSON 请求"))
            continue

        if request.get("action") == "shutdown":
            break

        image_path_str = request.get("image", "")
        project = request.get("project", "今日项目")
        image_path = Path(image_path_str).expanduser()

        if not image_path.exists():
            emit(make_error_response(f"图片不存在：{image_path}"))
            continue

        prompt = build_prompt(project)
        try:
            formatted_prompt = apply_chat_template(
                processor, model.config, prompt, num_images=1
            )
            response = generate(
                model,
                processor,
                prompt=formatted_prompt,
                image=[str(image_path)],
                max_tokens=256,
                temperature=0.0,
            )
        except Exception as exc:  # pragma: no cover
            emit(make_error_response(f"识别失败：{exc}"))
            continue

        text = getattr(response, "text", str(response)).strip()
        try:
            payload = parse_model_json(text)
        except json.JSONDecodeError:
            emit({
                "error": "parse_error",
                "intent": "模型返回了非 JSON 内容",
                "is_on_project": False,
                "confidence": 0.0,
                "reason": text[:500],
                "visible_context": "",
            })
            continue

        emit(normalize_payload(payload))

    return 0


def main() -> int:
    args = parse_args()
    if args.daemon:
        return run_daemon(args)
    if not args.image:
        print("Error: --image is required in one-shot mode", file=sys.stderr)
        return 1
    return run_once(args)


if __name__ == "__main__":
    raise SystemExit(main())
