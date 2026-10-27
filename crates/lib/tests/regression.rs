use std::fs::{File, create_dir, exists, read_dir};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::process::exit;

use either::Either;

use lari::*;

const PATTERNS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/patterns");
const ANSWERS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/answers");

const STEPS: [usize; 5] = [1, 2, 4, 8, 100];

struct Test {
    case: TestCase,
    status: TestStatus,
}

enum TestStatus {
    Pass,
    Fail,
}

struct TestCase {
    init: PathBuf,
    step: usize,
}
impl TestCase {
    fn init_file(&self) -> String {
        self.init
            .file_name()
            .expect("path to point to a file")
            .to_os_string()
            .into_string()
            .expect("uft8 encoded path")
    }

    fn answer_file(&self) -> String {
        let file = self.init_file();
        format!("{file}.{}", self.step)
    }

    fn init_path(&self) -> String {
        self.init
            .clone()
            .into_os_string()
            .into_string()
            .expect("uft8 encoded path")
    }

    fn answer_path(&self) -> String {
        format!("{ANSWERS}/{}", self.answer_file())
    }

    fn init_load(&self) -> io::Result<Grid> {
        load_grid(&self.init)
    }

    fn answer_load(&self) -> io::Result<Grid> {
        load_grid(self.answer_path())
    }

    fn name(&self) -> String {
        format!(
            "{}-{}",
            self.init_file()
                .rsplit_once('.')
                .expect("file name to have an extension")
                .0,
            self.step
        )
    }
}

impl std::fmt::Display for TestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TestStatus::Pass => "PASS",
                TestStatus::Fail => "FAIL",
            }
        )
    }
}

fn get_tests() -> impl Iterator<Item = io::Result<TestCase>> {
    read_dir(PATTERNS).into_iter().flat_map(|entry| {
        entry.flat_map(|entry| match entry {
            Ok(entry) => {
                let path = entry.path();
                Either::Left(STEPS.iter().map(move |&s| {
                    Ok(TestCase {
                        init: path.clone(),
                        step: s,
                    })
                }))
            }
            Err(err) => Either::Right(
                std::iter::from_fn(move || Some(Err(err.kind().into()))).take(STEPS.len()),
            ),
        })
    })
}

fn load_grid(path: impl AsRef<Path>) -> io::Result<Grid> {
    let path = path.as_ref();
    let mut file = File::open(path)?;

    rle::to_grid(&mut file)
}

fn create_answers(tests: Vec<TestCase>) -> io::Result<()> {
    let total = tests.len();
    for (i, test) in tests.into_iter().enumerate() {
        let mut grid = test.init_load()?;

        println!("[{i:>5}/{total}] creating {}...", test.name());
        grid.step(test.step);

        let mut out = File::create(test.answer_path())?;
        rle::from_grid(&grid, &mut out)?;
    }

    Ok(())
}

fn check(tests: Vec<TestCase>) -> impl Iterator<Item = Test> {
    let total = tests.len();

    let mut answers = vec![];
    for (i, test) in tests.into_iter().enumerate() {
        println!("loading... {i:>5}/{total}");
        let grid = match test.answer_load() {
            Ok(grid) => grid,
            Err(err) => {
                eprintln!(
                    "ERROR: unable to load answer for {} from {}\n{err}",
                    test.name(),
                    test.answer_path()
                );
                exit(1)
            }
        };
        answers.push((test, grid));
    }

    println!("sorting tests...");
    answers.sort_by_key(|a| a.1.alive().count());

    println!("evaluating tests...");
    answers
        .into_iter()
        .enumerate()
        .map(move |(i, (test, answer))| {
            let mut attempt = match test.init_load() {
                Ok(grid) => grid,
                Err(err) => {
                    eprintln!(
                        "ERROR: unable to load initial state for {} from {}\n{err}",
                        test.name(),
                        test.init_path()
                    );
                    exit(1)
                }
            };

            print!("[{i:>5}/{total}] testing {}...", test.name());
            attempt.step(test.step);

            Test {
                case: test,
                status: if attempt == answer {
                    TestStatus::Pass
                } else {
                    TestStatus::Fail
                },
            }
        })
}

#[test]
#[ignore]
fn regression() -> io::Result<()> {
    println!("loading tests...");
    let mut count = 0;
    let tests = match get_tests()
        .inspect(|test| {
            if let Ok(test) = test.as_ref() {
                count += 1;
                println!("{:<30}{count:>5}", test.name());
            }
        })
        .collect::<io::Result<Vec<_>>>()
    {
        Ok(tests) => tests,
        Err(err) => {
            panic!("ERROR: failed to read some tests from {PATTERNS}\n{err}");
        }
    };

    if !exists(ANSWERS)? {
        create_dir(ANSWERS)?;
        create_answers(tests)?;
    } else {
        let mut passed = vec![];
        let mut failed = vec![];
        for test in check(tests) {
            println!("{}", test.status);
            match test.status {
                TestStatus::Fail => {
                    failed.push(test.case);
                    break;
                }
                TestStatus::Pass => {
                    passed.push(test.case);
                }
            }
        }
        println!("PASSED: {} tests", passed.len());
        assert!(failed.is_empty());
    }

    Ok(())
}
