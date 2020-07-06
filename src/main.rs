use argh::FromArgs;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path;
use std::path::PathBuf;
use std::process::exit;
use subprocess::{Exec, ExitStatus};

#[derive(FromArgs)]
/// Harness the power of Grumpy to automate standard project creation and maintenance.
///
/// Requires the presence of cargo-edit on the running system.
struct GrumpyArgs {
    #[argh(subcommand)]
    sub_command: SubCommandEnum,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum SubCommandEnum {
    New(NewSubCommand),
    Add(AddSubCommand),
}

#[derive(FromArgs, PartialEq, Debug)]
/// create a new project
#[argh(subcommand, name = "new")]
struct NewSubCommand {
    /// name of project
    #[argh(positional)]
    project_name: String,

    /// create a binary-only project
    #[argh(switch, short = 'b')]
    bin_only: bool,

    /// create a library-only project
    #[argh(switch, short = 'l')]
    lib_only: bool,

    #[argh(option, short = 's')]
    /// what to call the executable script, defaults to main
    script_name: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug)]
/// add new binaries to an existing project
#[argh(subcommand, name = "add")]
struct AddSubCommand {
    /// name of project
    #[argh(option, short = 'p')]
    project_name: Option<String>,

    #[argh(positional)]
    /// what to call the executable script, defaults to main
    script_name: String,
}

fn get_project_path_buf(project_name: &String) -> PathBuf {
    path::PathBuf::from(env::current_dir().unwrap()).join(project_name)
}

struct ChangeWorkingDirectory {
    previous_directory: path::PathBuf,
}

impl ChangeWorkingDirectory {
    fn change(new_directory: &impl AsRef<path::Path>) -> Self {
        let current_working_directory = env::current_dir().unwrap();

        env::set_current_dir(new_directory).unwrap();

        ChangeWorkingDirectory {
            previous_directory: current_working_directory,
        }
    }
}

impl Drop for ChangeWorkingDirectory {
    fn drop(&mut self) {
        env::set_current_dir(&self.previous_directory).unwrap();
    }
}

struct CargoCommand {
    command: String,
    args: Vec<String>,
}

impl CargoCommand {
    fn new(command: &str) -> Self {
        CargoCommand {
            command: command.to_string(),
            args: vec![],
        }
    }

    fn add_arg(&mut self, arg: &str) -> &mut Self {
        self.args.push(arg.to_string());

        self
    }

    fn run(&self) -> i32 {
        let cargo_command = env::var("CARGO").unwrap();

        let mut command = Exec::cmd(cargo_command).arg(&self.command);

        for arg in &self.args {
            command = command.arg(arg);
        }

        match command.join().unwrap() {
            ExitStatus::Exited(0) => 0,
            ExitStatus::Exited(exit_code) => exit_code as i32,
            other => {
                println!("Unexpected error code {:?} found", other);
                100
            }
        }
    }
}

fn create_binary_script(project_name: &String, script_name: &String, overwrite: bool) -> i32 {
    let project_root = get_project_path_buf(project_name);
    let source_root = project_root.join("src");

    let mut filename: PathBuf;

    // We have one of two cases here. We're either in a lib project, in which case executable
    // scripts live under a bin subdirectory. Otherwise, we add directly to the current
    // project_root.
    if source_root.join("lib.rs").exists() {
        // Library, so we want to create any scripts under a bin subdirectory
        fs::create_dir_all(source_root.join("bin")).unwrap();

        let new_script_path = source_root.join("bin").join(script_name);

        if new_script_path.exists() {
            println!("Not creating {:?}, file already exists", new_script_path);
            return 102;
        } else {
            filename = new_script_path;
        };
    } else {
        // Binary project, add to the root
        let binary_source_file = source_root.join("main.rs");

        if binary_source_file.exists() {
            if overwrite {
                fs::remove_file(&binary_source_file).unwrap();
            } else {
                println!(
                    "Not overwriting {:?} in existing project, exiting",
                    binary_source_file
                );
                return 101;
            }
        }

        filename = binary_source_file;
    }

    filename.set_extension("rs");

    if filename.exists() {
        println!("Not creating {:?}, already exists", filename);
        return 102;
    } else {
        // Target script doesn't exist, we can create it now.
        let mut script = File::create(filename).unwrap();

        script
            .write(
                b"\
use anyhow::Error;

fn run() -> Result<(), Error> {
    println!(\"Hello, world!\");

    Ok(())
}

fn main() -> Result<(), Error> {
    run()?;
    Ok(())
}",
            )
            .unwrap();
    }

    {
        let _dir_change = ChangeWorkingDirectory::change(&project_root);

        // Now, we'll ensure that Cargo.toml contains the right crate dependencies.
        // We'll do this by making life easy on ourselves and using cargo-edit facilities to do
        // the addition.
        CargoCommand::new("add").add_arg("fehler@1.0").run();
        CargoCommand::new("add").add_arg("anyhow@1.0").run();
        CargoCommand::new("add").add_arg("thiserror@1.0").run();
        CargoCommand::new("add").add_arg("log@0.4").run();
        CargoCommand::new("add").add_arg("log4rs@0.8").run();
    }

    0
}

fn process_new(new_args: &NewSubCommand) -> i32 {
    let bin_only = new_args.bin_only;
    let lib_only = new_args.lib_only;

    if bin_only && lib_only {
        println!("Must only specify one of binary-only or library-only");
        return 1;
    }

    let mut cargo_command = CargoCommand::new("new");

    if new_args.bin_only {
        cargo_command.add_arg("--bin");
    } else {
        cargo_command.add_arg("--lib");
    }

    cargo_command.add_arg(new_args.project_name.as_str());

    match cargo_command.run() {
        0 => {}
        code => return code,
    }

    if !lib_only {
        return create_binary_script(
            &new_args.project_name,
            &new_args
                .script_name
                .as_ref()
                .unwrap_or(&"main.rs".to_string()),
            true,
        );
    }

    0
}

fn process_add(add_args: &AddSubCommand) -> i32 {
    if env::current_dir().unwrap().join("src").exists() {
        // We're probably inside an existing project, so we want to create something here
        // without specifying the project name.

        if add_args.project_name.is_some() {
            // Oops, project name was specified though, so this is probably an error.
            println!("Specified a project name but appear to be inside a project already");
            return 103;
        }
    } else if add_args.project_name.is_none() {
        // Not in a directory with a src folder, so we need a project name, but weren't given one.
        println!("No project name specified");
        return 104;
    }

    create_binary_script(
        &add_args.project_name.as_ref().unwrap_or(&".".to_string()),
        &add_args.script_name,
        false,
    )
}

fn main() {
    let args: GrumpyArgs = argh::cargo_from_env();

    let exit_code = match args.sub_command {
        SubCommandEnum::New(new_args) => process_new(&new_args),
        SubCommandEnum::Add(add_args) => process_add(&add_args),
    };

    exit(exit_code);
}
