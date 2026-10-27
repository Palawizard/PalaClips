export const SETTLE_MS = 120;

export interface ScrubMedia {
  playing(): boolean;
  hold(): void;
  paint(seconds: number): void;
  seek(seconds: number): boolean;
  release(resume: boolean): void;
}

export class ScrubSession {
  private active = false;
  private resume = false;
  private loading = false;
  private next: number | null = null;
  private releasing = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  position = 0;

  constructor(
    private media: ScrubMedia,
    private settleMs = SETTLE_MS,
  ) {}

  get scrubbing() {
    return this.active || this.releasing;
  }

  start(seconds: number) {
    if (!this.scrubbing) {
      this.resume = this.media.playing();
      this.media.hold();
    }
    this.active = true;
    this.releasing = false;
    this.move(seconds);
  }

  move(seconds: number) {
    if (!this.active) return;
    this.position = seconds;
    this.media.paint(seconds);
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.load(seconds), this.settleMs);
  }

  end() {
    if (!this.active) return;
    clearTimeout(this.timer);
    this.active = false;
    this.releasing = true;
    this.load(this.position);
    if (!this.loading) this.finish();
  }

  loaded() {
    this.loading = false;
    if (this.next !== null) {
      const seconds = this.next;
      this.next = null;
      this.load(seconds);
      if (this.loading) return;
    }
    if (this.releasing) this.finish();
  }

  dispose() {
    clearTimeout(this.timer);
  }

  private load(seconds: number) {
    if (this.loading) {
      this.next = seconds;
      return;
    }
    this.loading = this.media.seek(seconds);
  }

  private finish() {
    this.releasing = false;
    this.media.release(this.resume);
  }
}
