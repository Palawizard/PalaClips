import { convertFileSrc } from "@tauri-apps/api/core";
import { MEDIA_SCHEME } from "./ipc.ts";

const DRIFT_LIMIT_SECS = 0.04;
const DRIFT_CHECK_MS = 250;

export interface TrackMix {
  volume: number;
  mute: boolean;
  solo: boolean;
}

interface Voice {
  element: HTMLAudioElement;
  gain: GainNode;
}

export class PlayerAudio {
  private context = new AudioContext();
  private voices: Voice[] = [];
  private timer = 0;
  private held = false;

  constructor(private video: HTMLVideoElement) {
    video.addEventListener("play", this.play);
    video.addEventListener("pause", this.pause);
    video.addEventListener("seeking", this.align);
    video.addEventListener("ratechange", this.align);
  }

  load(paths: string[]) {
    this.unload();
    this.voices = paths.map((path) => {
      const element = new Audio();
      element.crossOrigin = "anonymous";
      element.preload = "auto";
      element.src = convertFileSrc(path, MEDIA_SCHEME);
      const gain = this.context.createGain();
      this.context.createMediaElementSource(element).connect(gain).connect(this.context.destination);
      return { element, gain };
    });
    this.align();
    if (!this.video.paused) this.play();
  }

  mix(tracks: TrackMix[]) {
    const solo = tracks.some((track) => track.solo);
    this.voices.forEach((voice, index) => {
      const track = tracks[index];
      const audible = track && !track.mute && (!solo || track.solo);
      voice.gain.gain.setTargetAtTime(audible ? track.volume : 0, this.context.currentTime, 0.015);
    });
  }

  hold() {
    this.held = true;
  }

  release() {
    this.held = false;
    this.align();
  }

  private play = () => {
    this.context.resume().catch(() => undefined);
    this.align();
    for (const voice of this.voices) voice.element.play().catch(() => undefined);
    window.clearInterval(this.timer);
    this.timer = window.setInterval(this.keepInSync, DRIFT_CHECK_MS);
  };

  private pause = () => {
    window.clearInterval(this.timer);
    for (const voice of this.voices) voice.element.pause();
    this.align();
  };

  private align = () => {
    if (this.held) return;
    for (const voice of this.voices) {
      voice.element.playbackRate = this.video.playbackRate;
      if (Math.abs(voice.element.currentTime - this.video.currentTime) > DRIFT_LIMIT_SECS) {
        voice.element.currentTime = this.video.currentTime;
      }
    }
  };

  private keepInSync = () => {
    if (this.video.paused) return;
    for (const voice of this.voices) {
      if (voice.element.paused && !this.video.ended) voice.element.play().catch(() => undefined);
    }
    this.align();
  };

  private unload() {
    window.clearInterval(this.timer);
    for (const voice of this.voices) {
      voice.element.pause();
      voice.element.removeAttribute("src");
      voice.element.load();
      voice.gain.disconnect();
    }
    this.voices = [];
  }

  dispose() {
    this.unload();
    this.video.removeEventListener("play", this.play);
    this.video.removeEventListener("pause", this.pause);
    this.video.removeEventListener("seeking", this.align);
    this.video.removeEventListener("ratechange", this.align);
    this.context.close().catch(() => undefined);
  }
}
