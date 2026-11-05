#[test]
fn pgn_numeric_and_symbolic_annotations_are_validated() {
    let game = parse("1.e4 ! $255 e5?! 2.Nf3 $0 Nc6 *").unwrap().remove(0);
    assert_eq!(game.moves().len(), 4);
    failure("1.e4 $256 *", PgnErrorKind::Syntax, 1, 2, "$256");
    failure(
        "1.e4!!! *",
        PgnErrorKind::Move(MoveError::Notation),
        1,
        1,
        "e4!!!",
    );
    failure("1. ! *", PgnErrorKind::Syntax, 1, 1, "!");
}

#[test]
fn pgn_export_comment_errors_preserve_the_ply_and_contents() {
    let mut game = played("e2e4");
    let text = "x".repeat(79);
    game.set_comment(1, &text).unwrap();
    let error = game.export().unwrap_err();
    assert_eq!(error.kind, PgnErrorKind::Comment);
    assert_eq!(
        (error.game, error.ply, error.token.as_str()),
        (1, 1, text.as_str())
    );
    let error = game.set_comment(2, "not a recorded ply").unwrap_err();
    assert_eq!(error.kind, PgnErrorKind::Comment);
    assert_eq!((error.game, error.ply), (1, 2));
}

use super::pgn::{MAX_INPUT_BYTES, MAX_VARIATION_DEPTH, PgnErrorKind, PgnGame, PgnResult, parse};
use super::{ChessState, INITIAL_FEN, MoveError};

fn played(sequence: &str) -> PgnGame {
    let mut game = PgnGame::new(&ChessState::initial());
    for uci in sequence.split_whitespace() {
        let chess_move = game.state().parse_uci(uci).unwrap();
        game.play(chess_move).unwrap();
    }
    game
}

#[test]
fn pgn_round_trip_preserves_moves_state_history_and_comments() {
    let mut game = played("e2e4 e7e5 g1f3 b8c6 f1b5 a7a6 b5c6 d7c6 e1g1");
    game.set_comment(0, "An original opening exercise.")
        .unwrap();
    game.set_comment(7, "Exchange the bishop for a knight.")
        .unwrap();
    let text = game.export().unwrap();
    let games = parse(&text).unwrap();
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].moves(), game.moves());
    assert_eq!(games[0].state(), game.state());
    assert_eq!(games[0].comment(0), game.comment(0));
    assert_eq!(games[0].comment(7), game.comment(7));
    assert!(text.contains("5. O-O *"));
}

#[test]
fn pgn_defaults_and_escaped_tags_are_exported_in_roster_order() {
    let mut game = played("e2e4");
    game.tags.insert("White".into(), "Ada \"A\" \\ Home".into());
    game.tags
        .insert("Black".into(), "Computer (Level 3)".into());
    let text = game.export().unwrap();
    let names: Vec<_> = text
        .lines()
        .take(7)
        .map(|s| s.split(' ').next().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "[Event", "[Site", "[Date", "[Round", "[White", "[Black", "[Result"
        ]
    );
    assert!(text.contains("[Event \"Chess3D\"]\n[Site \"?\"]"));
    assert!(text.contains("[Date \"????.??.??\"]\n[Round \"-\"]"));
    assert!(!text.contains("[SetUp"));
    let imported = parse(&text).unwrap().remove(0);
    assert_eq!(imported.tags["White"], game.tags["White"]);
    assert_eq!(imported.tags["Black"], "Computer (Level 3)");
}

#[test]
fn pgn_import_validates_variations_but_retains_only_the_main_line() {
    let text = "[Event \"Study\"]\n[Result \"*\"]\n\n\
        {Start} 1.e4 $1 {Center} (1.d4 d5 (1...Nf6) 2.c4) e5 ; reply\n\
        2.Nf3!? Nc6 3.Bb5 a6 *";
    let game = parse(text).unwrap().remove(0);
    let reference = played("e2e4 e7e5 g1f3 b8c6 f1b5 a7a6");
    assert_eq!(game.state(), reference.state());
    assert_eq!(game.moves(), reference.moves());
    assert!(game.comment(1).unwrap().contains("Center"));
    assert!(game.comment(2).unwrap().contains("reply"));
    assert_eq!(game.result, PgnResult::Unfinished);
}

#[test]
fn pgn_setup_preserves_black_turn_and_fullmove_number() {
    let fen = "4k3/8/8/3pP3/8/8/8/4K3 b - - 0 20";
    let mut game = PgnGame::new(&ChessState::from_fen(fen).unwrap());
    game.play(game.state().parse_uci("e8d7").unwrap()).unwrap();
    let text = game.export().unwrap();
    assert!(text.contains("[SetUp \"1\"]"));
    assert!(text.contains(&format!("[FEN \"{fen}\"]")));
    assert!(text.contains("20... Kd7 *"));
    let imported = parse(&text).unwrap().remove(0);
    assert_eq!(imported.initial_fen(), fen);
    assert_eq!(imported.state(), game.state());
}

#[test]
fn pgn_import_accepts_castle_promotion_and_optional_check_suffixes() {
    for (fen, moves, uci) in [
        ("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", "0-0", "e1g1"),
        ("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", "0-0-0", "e1c1"),
        ("7k/4P3/8/8/8/8/8/4K3 w - - 0 1", "e8Q", "e7e8q"),
        ("7k/4P3/8/8/8/8/8/4K3 w - - 0 1", "e8=Q+", "e7e8q"),
        ("7k/8/5KQ1/8/8/8/8/8 w - - 0 1", "Qg7", "g6g7"),
    ] {
        let text = format!("[SetUp \"1\"] [FEN \"{fen}\"] 1. {moves} *");
        let game = parse(&text).unwrap().remove(0);
        assert_eq!(game.moves()[0].to_uci(), uci);
    }
}

#[test]
fn pgn_authored_games_support_multiple_results_and_adjacent_games() {
    let text = "[Event \"Diagonal drill\"] 1. f3 e5 2. g4 Qh4# 0-1\n\
        [Event \"Development drill\"] 1. d4 d5 2. Nf3 Nf6 1/2-1/2\n\
        [Event \"Open file drill\"] 1. e4 d5 2. exd5 Qxd5 3. Nc3 *\n\
        1. e4 e5 1-0";
    let games = parse(text).unwrap();
    assert_eq!(games.len(), 4);
    assert_eq!(games[0].result, PgnResult::BlackWin);
    assert_eq!(games[1].result, PgnResult::Draw);
    assert_eq!(games[2].moves().len(), 5);
    assert_eq!(games[3].result, PgnResult::WhiteWin);
    for game in games {
        assert_eq!(
            parse(&game.export().unwrap()).unwrap()[0].state(),
            game.state()
        );
    }
}

#[test]
fn pgn_movetext_wraps_at_eighty_columns_including_comments() {
    let mut game = played("e2e4 e7e5 g1f3 b8c6 f1b5 a7a6 b5a4 g8f6 e1g1 f8e7 d2d3 d7d6");
    game.set_comment(1, &"A useful explanatory comment. ".repeat(20))
        .unwrap();
    let text = game.export().unwrap();
    let movetext = text.split_once("\n\n").unwrap().1;
    assert!(movetext.lines().all(|line| line.chars().count() <= 80));
    assert!(movetext.lines().count() > 2);
    assert_eq!(parse(&text).unwrap()[0].state(), game.state());
}

fn failure(text: &str, kind: PgnErrorKind, game: usize, ply: usize, token: &str) {
    let error = parse(text).unwrap_err();
    assert_eq!(error.kind, kind, "{error}");
    assert_eq!(
        (error.game, error.ply, error.token.as_str()),
        (game, ply, token)
    );
}

#[test]
fn pgn_move_errors_report_game_ply_and_the_original_token() {
    failure(
        "1.e4 * 1.d4 d5 2.Qh5",
        PgnErrorKind::Move(MoveError::Notation),
        2,
        3,
        "Qh5",
    );
    failure(
        "1.e4 (1.d4 e5 2.Qh5) *",
        PgnErrorKind::Move(MoveError::Notation),
        1,
        3,
        "Qh5",
    );
    failure(
        "1.e4 e5 2.Nf3+ *",
        PgnErrorKind::Move(MoveError::Notation),
        1,
        3,
        "Nf3+",
    );
}

#[test]
fn pgn_syntax_tag_setup_and_result_errors_are_typed() {
    failure("1.e4 ) *", PgnErrorKind::Syntax, 1, 2, ")");
    failure(
        "1.e4 {unfinished",
        PgnErrorKind::Syntax,
        1,
        2,
        "{unfinished",
    );
    failure("1.e4 $x *", PgnErrorKind::Syntax, 1, 2, "$x");
    failure(
        "[Event \"bad\\q\"] *",
        PgnErrorKind::Tag,
        1,
        1,
        "[Event \"bad\\q\"]",
    );
    failure(
        "[Event \"a\"] [Event \"b\"] *",
        PgnErrorKind::Tag,
        1,
        1,
        "Event",
    );
    failure("[SetUp \"1\"] *", PgnErrorKind::Tag, 1, 1, "SetUp");
    failure("[FEN \"bad\"] *", PgnErrorKind::Tag, 1, 1, "FEN");
    failure(
        "[SetUp \"1\"] [FEN \"bad\"] *",
        PgnErrorKind::Fen(super::FenError::Fields),
        1,
        1,
        "bad",
    );
    failure(
        "[Result \"1-0\"] 1.e4 *",
        PgnErrorKind::ResultMismatch,
        1,
        2,
        "*",
    );
    failure("[Result \"bad\"] *", PgnErrorKind::Tag, 1, 1, "bad");
    failure("1.e4", PgnErrorKind::MissingResult, 1, 2, "<eof>");
}

#[test]
fn pgn_bounds_input_and_recursive_variations() {
    failure(
        &" ".repeat(MAX_INPUT_BYTES + 1),
        PgnErrorKind::InputLimit,
        1,
        1,
        "<input>",
    );
    let nested = format!(
        "{}1.e4{} *",
        "1.e4 (".repeat(MAX_VARIATION_DEPTH + 1),
        ")".repeat(MAX_VARIATION_DEPTH + 1)
    );
    failure(&nested, PgnErrorKind::NestingLimit, 1, 2, "(");
    assert!(parse(" \n; no games\n").unwrap().is_empty());
}

#[test]
fn pgn_export_rejects_invalid_tags_comments_and_move_application() {
    let mut game = PgnGame::new(&ChessState::initial());
    assert_eq!(game.initial_fen(), INITIAL_FEN);
    let chess_move = game.state().parse_uci("e2e4").unwrap();
    game.play(chess_move).unwrap();
    let error = game.play(chess_move).unwrap_err();
    assert_eq!(error.kind, PgnErrorKind::Move(MoveError::IllegalMove));
    assert_eq!(
        (error.game, error.ply, error.token.as_str()),
        (1, 2, "e2e4")
    );
    let error = game.set_comment(1, "bad \0 comment").unwrap_err();
    assert_eq!(error.kind, PgnErrorKind::Comment);
    assert_eq!((error.ply, error.token.as_str()), (1, "bad \0 comment"));
    game.tags.insert("Bad tag".into(), "value".into());
    let error = game.export().unwrap_err();
    assert_eq!(error.kind, PgnErrorKind::Tag);
    assert_eq!((error.ply, error.token.as_str()), (1, "Bad tag"));
}

#[test]
fn pgn_semicolon_comment_braces_round_trip_without_injecting_movetext() {
    let game = parse("1.e4 ; ignore } * { and ( Qh5 )\ne5 *")
        .unwrap()
        .remove(0);
    let text = game.export().unwrap();
    let imported = parse(&text).unwrap().remove(0);
    assert_eq!(imported.state(), game.state());
    assert_eq!(imported.comment(1), game.comment(1));
}

#[test]
fn pgn_structural_errors_include_number_and_variation_context() {
    for (text, ply, token) in [
        ("1. *", 1, "*"),
        ("1. 1.e4 *", 1, "1."),
        ("1.e4 (1.d4 1...) *", 2, ")"),
        ("1.e4 () *", 1, ")"),
        ("1.e4 (1.d4", 2, "<eof>"),
        ("1.e4 (1.d4 1-0 e5) *", 2, "e5"),
        ("1.e4 2...e5 *", 2, "2..."),
        ("1.e4 (", 1, "<eof>"),
    ] {
        failure(text, PgnErrorKind::Syntax, 1, ply, token);
    }
}

#[test]
fn pgn_accepts_the_exact_size_and_nesting_limits() {
    let text = format!("*{}", " ".repeat(MAX_INPUT_BYTES - 1));
    assert_eq!(parse(&text).unwrap().len(), 1);
    let nested = format!(
        "{}1.e4{} *",
        "1.e4 (".repeat(MAX_VARIATION_DEPTH),
        ")".repeat(MAX_VARIATION_DEPTH)
    );
    assert_eq!(parse(&nested).unwrap()[0].moves().len(), 1);
}

#[test]
fn pgn_counter_overflow_is_reported_at_the_move() {
    let text = "[SetUp \"1\"] [FEN \"4k3/8/8/8/8/8/8/R3K3 w - - 4294967295 1\"] 1.Ra2 *";
    failure(
        text,
        PgnErrorKind::Move(MoveError::CounterOverflow),
        1,
        1,
        "Ra2",
    );
}

#[test]
fn pgn_restores_repetition_and_en_passant_history() {
    let game = played("g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1 f6g8");
    let imported = parse(&game.export().unwrap()).unwrap().remove(0);
    assert!(imported.state().draw_claims().threefold);
    assert_eq!(imported.state(), game.state());
    let game = parse("[SetUp \"1\"] [FEN \"4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 20\"] 20.exd6 *")
        .unwrap()
        .remove(0);
    assert_eq!(game.moves()[0].to_uci(), "e5d6");
    assert_eq!(
        parse(&game.export().unwrap()).unwrap()[0].state(),
        game.state()
    );
}
