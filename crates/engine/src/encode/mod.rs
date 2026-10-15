//! Encodage — FFmpeg via `rsmpeg` (voir `docs/DECISIONS.md` ADR-004).
//!
//! **Phase 1** (vidéo) puis **phase 2** (audio).
//!
//! - Vidéo : `h264_nvenc` / `hevc_nvenc` / `av1_nvenc` (NVIDIA), `*_amf` (AMD),
//!   `*_qsv` (Intel). Détection au démarrage par un **encodage de test réel**,
//!   pas seulement par la présence du codec.
//! - Entrée : textures D3D11 (frames matérielles `d3d11va`), conversion BGRA → NV12
//!   sur le GPU. Aucune copie CPU dans le chemin nominal.
//! - Débit constant (CBR) pour le replay, intervalle d'images clés configurable.
//! - Audio : AAC-LC 48 kHz stéréo, un encodeur par piste. Gérer le délai d'amorçage
//!   (priming) de l'AAC pour ne pas décaler l'audio (voir `docs/RESEARCH.md`, OBS).
//! - Une session NVENC unique pour replay + enregistrement manuel (les paquets
//!   sont partagés entre le buffer et le fichier d'enregistrement).
