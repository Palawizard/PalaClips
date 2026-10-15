//! Capture audio — WASAPI via le crate `wasapi`.
//!
//! **Phases 2 et 3** (voir `docs/PLAN.md`).
//!
//! Pistes, dans cet ordre dans le fichier (CLAUDE.md, règle 8) :
//! 1. Mix complet : loopback du périphérique de sortie (par défaut, suivi à chaud).
//! 2. Micro : périphérique d'entrée choisi, gain appliqué.
//! 3. Une piste par application : `AudioClient::new_application_loopback_client(pid, true)`
//!    sur le **PID parent** (navigateurs et Discord sont multi-processus).
//!
//! Règles :
//! - Horodatage par `pu64QPCPosition` uniquement (position device parfois à 0 en
//!   process loopback).
//! - Combler les trous par du silence calculé d'après les écarts QPC ; traiter
//!   `AUDCLNT_BUFFERFLAGS_SILENT` comme du silence ; ne **jamais** arrêter une piste
//!   sur timeout (contrairement à l'exemple `record_application`).
//! - Rattacher automatiquement une piste à son appli quand celle-ci (re)démarre :
//!   on suit les processus par nom d'exe.
//! - Détecter les pistes à zéro numérique alors que la session audio de l'appli
//!   montre du signal (`get_audiometerinformation`) → alerte `app_track_silent`.
//! - Format de capture : 48 kHz, stéréo, float32, `autoconvert = true`.
//!
//! Sélecteur : énumération des sessions actives via `get_iaudiosessionmanager()` →
//! `get_audiosessionenumerator()` → `get_process_id()` + crête en direct.
