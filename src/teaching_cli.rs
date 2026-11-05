use chess3d::teaching::{Course, MoveNode, PuzzlePack, Step};
use std::error::Error;

pub fn inspect(options: &crate::cli::Options) -> Result<bool, Box<dyn Error>> {
    if let Some(id) = &options.lesson {
        let course = Course::bundled()?;
        let lesson = course
            .lesson(id)
            .ok_or_else(|| format!("unknown lesson {id}"))?;
        println!(
            "{}: {}\nPrerequisites: {}",
            lesson.id,
            lesson.title,
            lesson.requires.join(", ")
        );
        for (index, step) in lesson.steps.iter().enumerate() {
            println!("\nStep {}", index + 1);
            match step {
                Step::Explain { text, .. } => println!("{text}"),
                Step::Task(task) => {
                    println!(
                        "{}\nFEN: {}\nSide: {:?}\nGoal: {:?}",
                        task.text, task.fen, task.side, task.goal
                    );
                    print_lines(&task.solutions, &[]);
                    for (level, hint) in task.hints.iter().enumerate() {
                        println!("Hint {}: {hint}", level + 1);
                    }
                }
            }
        }
        println!("\nCourse validated.");
        return Ok(true);
    }
    if let Some(id) = &options.puzzle {
        let pack = PuzzlePack::bundled()?;
        let puzzle = pack
            .puzzle(id)
            .ok_or_else(|| format!("unknown puzzle {id}"))?;
        let moves: Vec<_> = puzzle.moves.split_whitespace().collect();
        println!(
            "Puzzle {} ({}): {}",
            puzzle.id, puzzle.rating, puzzle.themes
        );
        println!(
            "Setup: {}\nFEN after setup: {}",
            moves[0],
            puzzle.after_setup()?.to_fen()
        );
        println!(
            "Solution: {}\nSource: {}\nPack validated.",
            moves[1..].join(" "),
            chess3d::teaching::PUZZLE_SOURCE
        );
        return Ok(true);
    }
    Ok(false)
}

fn print_lines(nodes: &[MoveNode], prefix: &[String]) {
    for node in nodes {
        let mut line = prefix.to_vec();
        line.push(node.uci.clone());
        if node.next.is_empty() {
            println!("Accepted line: {}", line.join(" "));
        } else {
            print_lines(&node.next, &line);
        }
    }
}
