use super::Weights;

/// Preferences only break close centipawn choices (at most 12 points). They do
/// not change mate ordering, force exchanges or prescribe a sequence of moves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Style {
    Development,
    Initiative,
    Exchanges,
    Structure,
    Balanced,
}

/// Uncalibrated opponent controls, not Elo ratings. Search attention is distinct
/// from bounded seeded policy noise. Material attention remains full in the
/// supplied ladder. Callers may tune budgets without changing an identity.
#[derive(Clone, Copy, Debug)]
pub struct OpponentProfile {
    pub name: &'static str,
    pub description: &'static str,
    pub nodes: u64,
    pub candidates: usize,
    pub weights: Weights,
    pub perturbation: u16,
    pub style: Option<Style>,
    pub depth: u8,
}

const fn profile(
    identity: (&'static str, &'static str),
    nodes: u64,
    candidates: usize,
    weights: Weights,
    perturbation: u16,
    style: Style,
) -> OpponentProfile {
    OpponentProfile {
        name: identity.0,
        description: identity.1,
        nodes,
        candidates,
        weights,
        perturbation,
        style: Some(style),
        depth: if nodes < 5000 {
            2
        } else if nodes < 50_000 {
            3
        } else {
            4
        },
    }
}

/// Pip favors direct captures and development; low positional attention misses
/// indirect threats. Immediate queen captures are screened at every level.
pub const PIP: OpponentProfile = profile(
    ("Pip", "Direct captures and development; shallow planning."),
    1200,
    2,
    Weights {
        material: 100,
        placement: 30,
        mobility: 10,
        king_safety: 20,
        pawns: 0,
    },
    65,
    Style::Development,
);
/// Rowan attends more to king cover and piece activity, with shallow tactical
/// reach. No rating or reliably greater playing strength has been established.
pub const ROWAN: OpponentProfile = profile(
    ("Rowan", "Piece activity and king cover; shallow tactics."),
    2400,
    3,
    Weights {
        material: 100,
        placement: 50,
        mobility: 30,
        king_safety: 65,
        pawns: 15,
    },
    50,
    Style::Development,
);
/// Mira emphasizes initiative and activity while paying little attention to
/// pawn defects. Its larger budget remains a design target, not a rating.
pub const MIRA: OpponentProfile = profile(
    (
        "Mira",
        "Initiative and activity; less care for pawn defects.",
    ),
    5000,
    3,
    Weights {
        material: 100,
        placement: 65,
        mobility: 120,
        king_safety: 70,
        pawns: 20,
    },
    40,
    Style::Initiative,
);
/// Ellis develops steadily, preferring roughly equal exchanges among close
/// choices. Intermediate moves can remain beyond its completed iterations.
pub const ELLIS: OpponentProfile = profile(
    ("Ellis", "Steady development and roughly equal exchanges."),
    9000,
    4,
    Weights {
        material: 100,
        placement: 80,
        mobility: 70,
        king_safety: 90,
        pawns: 50,
    },
    30,
    Style::Exchanges,
);
/// Nia favors activity and open lines with less pawn-structure attention. Its
/// tactical reach depends on the actual position and measured completed depth.
pub const NIA: OpponentProfile = profile(
    ("Nia", "Active pieces and open lines before pawn structure."),
    18_000,
    4,
    Weights {
        material: 100,
        placement: 90,
        mobility: 125,
        king_safety: 95,
        pawns: 65,
    },
    24,
    Style::Initiative,
);
/// Tomas values pawn structure and endgames with the complete evaluator. The
/// patient preference adds only a small bias to otherwise close choices.
pub const TOMAS: OpponentProfile = profile(
    ("Tomas", "Patient pawn structure and endgame play."),
    32_000,
    5,
    Weights {
        material: 100,
        placement: 100,
        mobility: 90,
        king_safety: 100,
        pawns: 120,
    },
    18,
    Style::Structure,
);
/// Imani balances activity and safety with small seeded variation. Prophylaxis
/// and practical complications are intended tendencies, not measured guarantees.
pub const IMANI: OpponentProfile = profile(
    (
        "Imani",
        "Balanced activity and safety with small variation.",
    ),
    60_000,
    5,
    Weights::BALANCED,
    10,
    Style::Balanced,
);
/// Soren uses the largest supplied budget and near-best variation. No scheduled
/// mistakes or claimed rating; strongest-ladder ordering still needs calibration.
pub const SOREN: OpponentProfile = profile(
    ("Soren", "Largest search budget and near-best variation."),
    120_000,
    6,
    Weights::BALANCED,
    4,
    Style::Balanced,
);

/// Eight named, uncalibrated configurations in intended increasing strength.
/// No numerical ratings should be published from these settings alone.
pub const LADDER: [OpponentProfile; 8] = [PIP, ROWAN, MIRA, ELLIS, NIA, TOMAS, IMANI, SOREN];
