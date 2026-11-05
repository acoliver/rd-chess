use super::{Finish, Game, MoveStats};
use crate::Side;

#[derive(Clone, Debug)]
pub struct MatchReport {
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    /// Score and interval for the second profile; a draw scores half a point.
    pub score: f64,
    /// Distribution-free 95% Hoeffding interval using independent colour pairs
    /// as the sampling unit. Conservative with draws; no Elo interpretation.
    pub interval: [f64; 2],
    pub stats: [MoveStats; 2],
    pub capped: u32,
}

impl MatchReport {
    pub fn new(games: &[Game]) -> Self {
        assert!(
            !games.is_empty() && games.len().is_multiple_of(2),
            "paired games required"
        );
        let mut report = Self {
            wins: 0,
            draws: 0,
            losses: 0,
            score: 0.0,
            interval: [0.0; 2],
            stats: [MoveStats::default(); 2],
            capped: 0,
        };
        for (index, game) in games.iter().enumerate() {
            assert_eq!(
                usize::try_from(game.index).expect("index fits usize"),
                index
            );
            match game.finish {
                Finish::Mate(winner) => {
                    let second_is_white = game.index % 2 == 1;
                    if (winner == Side::Ivory) == second_is_white {
                        report.wins += 1;
                    } else {
                        report.losses += 1;
                    }
                }
                Finish::RulesDraw | Finish::ClaimDraw | Finish::MoveCap => report.draws += 1,
            }
            report.capped += u32::from(game.finish == Finish::MoveCap);
            for (total, sample) in report.stats.iter_mut().zip(game.stats) {
                total.merge(sample);
            }
        }
        let count = f64::from(report.wins + report.draws + report.losses);
        report.score = (f64::from(report.wins) + f64::from(report.draws) / 2.0) / count;
        let radius = (40_f64.ln() / count).sqrt();
        report.interval = [
            (report.score - radius).max(0.0),
            (report.score + radius).min(1.0),
        ];
        report
    }
}
