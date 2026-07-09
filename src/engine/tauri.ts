import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type { EngineClient } from "./client";
import type { AudioProps, Id, StateSnapshot, TimeUs, Transform2D } from "./types";

export function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export class TauriEngine implements EngineClient {
  readonly kind = "tauri" as const;

  getState(): Promise<StateSnapshot> {
    return invoke("get_state");
  }
  splitClip(clipId: Id, tUs: TimeUs): Promise<StateSnapshot> {
    return invoke("split_clip", { clipId, tUs });
  }
  deleteClips(ids: Id[], ripple: boolean): Promise<StateSnapshot> {
    return invoke("delete_clips", { ids, ripple });
  }
  moveClip(
    clipId: Id,
    toTrack: Id,
    toStartUs: TimeUs,
    overwrite: boolean,
  ): Promise<StateSnapshot> {
    return invoke("move_clip", { clipId, toTrack, toStartUs, overwrite });
  }
  trimClip(clipId: Id, left: boolean, newEdgeUs: TimeUs): Promise<StateSnapshot> {
    return invoke("trim_clip", { clipId, left, newEdgeUs });
  }
  undo(): Promise<StateSnapshot> {
    return invoke("undo");
  }
  redo(): Promise<StateSnapshot> {
    return invoke("redo");
  }
  setClipAudio(clipId: Id, audio: AudioProps): Promise<StateSnapshot> {
    return invoke("set_clip_audio", { clipId, audio });
  }
  setClipTransform(clipId: Id, transform: Transform2D): Promise<StateSnapshot> {
    return invoke("set_clip_transform", { clipId, transform });
  }

  async pickMediaFiles(): Promise<string[] | null> {
    const picked = await open({
      multiple: true,
      title: "Importar medios",
      filters: [
        {
          name: "Medios",
          extensions: [
            "mp4", "mov", "mkv", "webm", "avi", "m4v", "mts", "mpg",
            "wav", "mp3", "m4a", "aac", "flac", "ogg", "aiff",
            "png", "jpg", "jpeg", "webp", "bmp", "tiff", "gif",
          ],
        },
      ],
    });
    if (!picked) return null;
    return Array.isArray(picked) ? picked : [picked];
  }

  importMedia(paths: string[]): Promise<StateSnapshot> {
    return invoke("import_media", { paths });
  }
  addClip(assetId: Id, atUs: TimeUs): Promise<StateSnapshot> {
    return invoke("add_clip", { assetId, atUs });
  }

  async renderFrame(tUs: TimeUs, maxWidth: number): Promise<Uint8Array | null> {
    const buf = await invoke<ArrayBuffer>("render_frame", { tUs, maxWidth });
    const bytes = new Uint8Array(buf);
    return bytes.length > 0 ? bytes : null;
  }
}
