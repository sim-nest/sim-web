//! On-screen performance keyboard Scene descriptors.
//!
//! The keyboard is a pure Scene descriptor: it names the bound performance
//! source, player chain, instrument, and stream bridge route, then leaves
//! interaction to the browser Intent emitter.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};
use sim_value::build::{int, list, text, uint};

/// Stable lens id for the on-screen performance keyboard.
pub const PERFORMANCE_KEYBOARD_VIEW_ID: &str = "view:performance-keyboard";

/// Demo fixture name for the bound player-chain keyboard.
pub const PERFORMANCE_KEYBOARD_DEMO_FIXTURE: &str = "player-chain-instrument";

/// Where browser keyboard gestures are sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceKeyboardBinding {
    /// Intent target: the runtime performance source that accepts events.
    pub target: Symbol,
    /// Performance event source id.
    pub source: Symbol,
    /// Browser or MIDI input id.
    pub input: Symbol,
    /// Player chain that receives source output.
    pub player_chain: Symbol,
    /// Instrument target at the end of the chain.
    pub instrument: Symbol,
    /// Browser stream bridge route used by the shell.
    pub stream: Symbol,
    /// MIDI channel, zero based.
    pub channel: u8,
}

impl PerformanceKeyboardBinding {
    /// Build the standard browser keyboard binding.
    pub fn browser(player_chain: Symbol, instrument: Symbol) -> Self {
        Self {
            target: Symbol::qualified("music/performance-source", "keyboard"),
            source: Symbol::qualified("music/performance-source", "keyboard"),
            input: Symbol::qualified("midi/input", "keyboard"),
            player_chain,
            instrument,
            stream: Symbol::qualified("stream/browser", "performance-keyboard"),
            channel: 0,
        }
    }
}

/// Serializable physical-key mapping for browser performance input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceKeyMap {
    /// Stable map name.
    pub name: String,
    /// Whether the browser editor may rewrite and persist this map.
    pub editable: bool,
    /// Physical keys and their performance actions.
    pub entries: Vec<PerformanceKeyMapEntry>,
    /// Current key velocity used by note entries.
    pub velocity: u8,
    /// Current transpose, in semitones.
    pub transpose: i8,
    /// Whether degree entries should be scale-locked by the browser.
    pub scale_lock: bool,
}

impl Default for PerformanceKeyMap {
    fn default() -> Self {
        Self::qwerty_two_row()
    }
}

impl PerformanceKeyMap {
    /// Default two-row chromatic map with controls for live performance.
    pub fn qwerty_two_row() -> Self {
        let mut entries = Vec::new();
        push_degree_row(&mut entries, &LOWER_ROW, 0);
        push_degree_row(&mut entries, &UPPER_ROW, 1);
        entries.extend([
            PerformanceKeyMapEntry::new("Space", " ", "Sustain", PerformanceKeyAction::Sustain),
            PerformanceKeyMapEntry::new(
                "BracketLeft",
                "[",
                "Octave down",
                PerformanceKeyAction::OctaveShift { amount: -1 },
            ),
            PerformanceKeyMapEntry::new(
                "BracketRight",
                "]",
                "Octave up",
                PerformanceKeyAction::OctaveShift { amount: 1 },
            ),
            PerformanceKeyMapEntry::new(
                "Comma",
                ",",
                "Transpose down",
                PerformanceKeyAction::Transpose { amount: -1 },
            ),
            PerformanceKeyMapEntry::new(
                "Period",
                ".",
                "Transpose up",
                PerformanceKeyAction::Transpose { amount: 1 },
            ),
            PerformanceKeyMapEntry::new(
                "Backslash",
                "\\",
                "Scale lock",
                PerformanceKeyAction::ScaleLock,
            ),
            PerformanceKeyMapEntry::new("Escape", "Esc", "Panic", PerformanceKeyAction::Panic),
            PerformanceKeyMapEntry::new(
                "F1",
                "F1",
                "Velocity low",
                PerformanceKeyAction::Velocity { value: 40 },
            ),
            PerformanceKeyMapEntry::new(
                "F2",
                "F2",
                "Velocity medium",
                PerformanceKeyAction::Velocity { value: 72 },
            ),
            PerformanceKeyMapEntry::new(
                "F3",
                "F3",
                "Velocity strong",
                PerformanceKeyAction::Velocity { value: 100 },
            ),
            PerformanceKeyMapEntry::new(
                "F4",
                "F4",
                "Velocity full",
                PerformanceKeyAction::Velocity { value: 127 },
            ),
        ]);
        Self {
            name: "qwerty-two-row".to_owned(),
            editable: true,
            entries,
            velocity: 96,
            transpose: 0,
            scale_lock: false,
        }
    }
}

/// A single physical key action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceKeyMapEntry {
    /// KeyboardEvent.code value.
    pub code: String,
    /// KeyboardEvent.key value when available.
    pub key: String,
    /// Short display label.
    pub label: String,
    /// Performance action triggered by the key.
    pub action: PerformanceKeyAction,
}

impl PerformanceKeyMapEntry {
    /// Build a physical key binding.
    pub fn new(
        code: impl Into<String>,
        key: impl Into<String>,
        label: impl Into<String>,
        action: PerformanceKeyAction,
    ) -> Self {
        Self {
            code: code.into(),
            key: key.into(),
            label: label.into(),
            action,
        }
    }
}

/// Browser-side performance action for a physical key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerformanceKeyAction {
    /// Map to a chromatic degree from the keyboard base.
    Degree {
        /// Scale degree relative to the keyboard base.
        degree: i8,
        /// Octave offset applied to the degree.
        octave: i8,
    },
    /// Map directly to a MIDI note number.
    Note {
        /// MIDI note number to emit.
        midi: i32,
    },
    /// Hold or release sustain.
    Sustain,
    /// Shift emitted notes by whole octaves.
    OctaveShift {
        /// Number of octaves to shift, signed.
        amount: i8,
    },
    /// Shift emitted notes by semitones.
    Transpose {
        /// Number of semitones to shift, signed.
        amount: i8,
    },
    /// Release all held notes.
    Panic,
    /// Toggle scale-locking for degree entries.
    ScaleLock,
    /// Set note-on velocity for later key presses.
    Velocity {
        /// Velocity value to apply to later notes.
        value: u8,
    },
}

/// Display state for the keyboard surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceKeyboardState {
    /// First displayed MIDI note before octave shift.
    pub base_midi: i32,
    /// Number of octaves to display.
    pub octaves: u8,
    /// Octave shift applied to emitted notes.
    pub octave_shift: i8,
    /// Pitch classes highlighted as in-scale.
    pub scale_lock: Vec<u8>,
    /// Notes currently held by the source.
    pub held_notes: Vec<i32>,
    /// Notes generated by downstream players.
    pub generated_notes: Vec<i32>,
    /// Sustain toggle state.
    pub sustain: bool,
    /// Current pitch-bend value in MIDI 14-bit range.
    pub pitch_bend: u16,
    /// Physical-key mapping for computer-keyboard performance.
    pub key_map: PerformanceKeyMap,
}

impl Default for PerformanceKeyboardState {
    fn default() -> Self {
        Self {
            base_midi: 48,
            octaves: 3,
            octave_shift: 0,
            scale_lock: vec![0, 2, 4, 5, 7, 9, 11],
            held_notes: Vec::new(),
            generated_notes: Vec::new(),
            sustain: false,
            pitch_bend: 8192,
            key_map: PerformanceKeyMap::default(),
        }
    }
}

/// Render the keyboard as a single `scene/keyboard` node.
pub fn performance_keyboard_view(
    binding: &PerformanceKeyboardBinding,
    state: &PerformanceKeyboardState,
) -> Expr {
    let shifted_base = state.base_midi + i32::from(state.octave_shift) * 12;
    node(
        "keyboard",
        vec![
            ("lens", sym(PERFORMANCE_KEYBOARD_VIEW_ID)),
            ("role", sym("performance-keyboard")),
            ("label", text("On-screen keyboard")),
            ("target", Expr::Symbol(binding.target.clone())),
            ("source", Expr::Symbol(binding.source.clone())),
            ("input", Expr::Symbol(binding.input.clone())),
            ("channel", uint(u64::from(binding.channel))),
            ("base-midi", int(i64::from(shifted_base))),
            ("octaves", uint(u64::from(state.octaves))),
            ("octave-shift", int(i64::from(state.octave_shift))),
            ("sustain", Expr::Bool(state.sustain)),
            ("pitch-bend", uint(u64::from(state.pitch_bend))),
            ("key-map", key_map_expr(&state.key_map)),
            (
                "scale-lock",
                list(
                    state
                        .scale_lock
                        .iter()
                        .map(|note| uint(u64::from(*note)))
                        .collect(),
                ),
            ),
            (
                "held-notes",
                list(
                    state
                        .held_notes
                        .iter()
                        .map(|note| int(i64::from(*note)))
                        .collect(),
                ),
            ),
            (
                "generated-notes",
                list(
                    state
                        .generated_notes
                        .iter()
                        .map(|note| int(i64::from(*note)))
                        .collect(),
                ),
            ),
            ("binding", binding_expr(binding)),
            ("keys", list(keys(shifted_base, state))),
        ],
    )
}

/// A deterministic shell demo binding the keyboard through a player chain to a
/// SUP instrument descriptor.
pub fn performance_keyboard_demo_scene() -> Expr {
    let binding = PerformanceKeyboardBinding::browser(
        Symbol::qualified("music/player-chain", "onscreen-keyboard"),
        Symbol::qualified("audio-synth/instrument", "dx7"),
    );
    let state = PerformanceKeyboardState {
        held_notes: vec![60, 64],
        generated_notes: vec![67, 72],
        sustain: true,
        ..PerformanceKeyboardState::default()
    };
    performance_keyboard_view(&binding, &state)
}

fn binding_expr(binding: &PerformanceKeyboardBinding) -> Expr {
    data_map(vec![
        ("target", Expr::Symbol(binding.target.clone())),
        ("source", Expr::Symbol(binding.source.clone())),
        ("input", Expr::Symbol(binding.input.clone())),
        ("player-chain", Expr::Symbol(binding.player_chain.clone())),
        ("instrument", Expr::Symbol(binding.instrument.clone())),
        ("stream", Expr::Symbol(binding.stream.clone())),
        ("channel", uint(u64::from(binding.channel))),
    ])
}

fn key_map_expr(key_map: &PerformanceKeyMap) -> Expr {
    data_map(vec![
        ("name", text(key_map.name.clone())),
        ("editable", Expr::Bool(key_map.editable)),
        ("velocity", uint(u64::from(key_map.velocity))),
        ("transpose", int(i64::from(key_map.transpose))),
        ("scale-lock", Expr::Bool(key_map.scale_lock)),
        (
            "entries",
            list(key_map.entries.iter().map(key_map_entry_expr).collect()),
        ),
    ])
}

fn key_map_entry_expr(entry: &PerformanceKeyMapEntry) -> Expr {
    let mut fields = vec![
        ("code", text(entry.code.clone())),
        ("key", text(entry.key.clone())),
        ("label", text(entry.label.clone())),
    ];
    match entry.action {
        PerformanceKeyAction::Degree { degree, octave } => {
            fields.extend([
                ("action", text("degree")),
                ("degree", int(i64::from(degree))),
                ("octave", int(i64::from(octave))),
            ]);
        }
        PerformanceKeyAction::Note { midi } => {
            fields.extend([("action", text("note")), ("midi", int(i64::from(midi)))]);
        }
        PerformanceKeyAction::Sustain => fields.push(("action", text("sustain"))),
        PerformanceKeyAction::OctaveShift { amount } => {
            fields.extend([
                ("action", text("octave-shift")),
                ("amount", int(i64::from(amount))),
            ]);
        }
        PerformanceKeyAction::Transpose { amount } => {
            fields.extend([
                ("action", text("transpose")),
                ("amount", int(i64::from(amount))),
            ]);
        }
        PerformanceKeyAction::Panic => fields.push(("action", text("panic"))),
        PerformanceKeyAction::ScaleLock => fields.push(("action", text("scale-lock"))),
        PerformanceKeyAction::Velocity { value } => {
            fields.extend([
                ("action", text("velocity")),
                ("value", uint(u64::from(value))),
            ]);
        }
    }
    data_map(fields)
}

fn keys(base_midi: i32, state: &PerformanceKeyboardState) -> Vec<Expr> {
    let count = usize::from(state.octaves) * 12;
    (0..count)
        .map(|offset| {
            let midi = base_midi + offset as i32;
            let class = midi.rem_euclid(12) as u8;
            data_map(vec![
                ("midi", int(i64::from(midi))),
                ("label", text(note_label(midi))),
                ("white", Expr::Bool(is_white_key(class))),
                ("scale", Expr::Bool(state.scale_lock.contains(&class))),
                ("held", Expr::Bool(state.held_notes.contains(&midi))),
                (
                    "generated",
                    Expr::Bool(state.generated_notes.contains(&midi)),
                ),
            ])
        })
        .collect()
}

const LOWER_ROW: [(&str, &str, &str, i8); 12] = [
    ("KeyZ", "z", "Z", 0),
    ("KeyS", "s", "S", 1),
    ("KeyX", "x", "X", 2),
    ("KeyD", "d", "D", 3),
    ("KeyC", "c", "C", 4),
    ("KeyV", "v", "V", 5),
    ("KeyG", "g", "G", 6),
    ("KeyB", "b", "B", 7),
    ("KeyH", "h", "H", 8),
    ("KeyN", "n", "N", 9),
    ("KeyJ", "j", "J", 10),
    ("KeyM", "m", "M", 11),
];

const UPPER_ROW: [(&str, &str, &str, i8); 12] = [
    ("KeyQ", "q", "Q", 0),
    ("Digit2", "2", "2", 1),
    ("KeyW", "w", "W", 2),
    ("Digit3", "3", "3", 3),
    ("KeyE", "e", "E", 4),
    ("KeyR", "r", "R", 5),
    ("Digit5", "5", "5", 6),
    ("KeyT", "t", "T", 7),
    ("Digit6", "6", "6", 8),
    ("KeyY", "y", "Y", 9),
    ("Digit7", "7", "7", 10),
    ("KeyU", "u", "U", 11),
];

fn push_degree_row(
    entries: &mut Vec<PerformanceKeyMapEntry>,
    row: &[(&str, &str, &str, i8)],
    octave: i8,
) {
    entries.extend(row.iter().map(|(code, key, label, degree)| {
        PerformanceKeyMapEntry::new(
            *code,
            *key,
            *label,
            PerformanceKeyAction::Degree {
                degree: *degree,
                octave,
            },
        )
    }));
}

fn is_white_key(class: u8) -> bool {
    matches!(class, 0 | 2 | 4 | 5 | 7 | 9 | 11)
}

fn note_label(midi: i32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let class = midi.rem_euclid(12) as usize;
    let octave = midi.div_euclid(12) - 1;
    format!("{}{}", NAMES[class], octave)
}
