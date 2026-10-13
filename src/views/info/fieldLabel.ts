import type { TrackField } from "@/ipc/types";

/** What each field is called, for the line the status bar says after a save. */
export const FIELD_LABEL: Record<TrackField, string> = {
  title: "Track Title",
  artist: "Artist",
  album: "Album",
  year: "Year",
  trackNumber: "Track number",
  discNumber: "Disc number",
  originalArtist: "Original Artist",
  composer: "Composer",
  remixer: "Remixer",
  lyricist: "Lyricist",
  playCount: "DJ Play Count",
  genre: "Genre",
  label: "Label",
  key: "Key",
  bpm: "BPM",
};
