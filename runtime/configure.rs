use std::path::Path;

use ninja_writer::*;

fn main() {
    let ninja = Ninja::new();

    let src = Path::new("src").canonicalize().unwrap();
    let include = Path::new("include").canonicalize().unwrap();
    let build = Path::new("build");
    if !build.exists() {
        std::fs::create_dir_all(build).unwrap();
    }
    let build = build.canonicalize().unwrap();

    let devkitpro = match std::env::var("DEVKITPRO") {
        Ok(val) if !val.is_empty() => val,
        Ok(_) => {
            eprintln!("DEVKITPRO is empty");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Cannot get DEVKITPRO: {}", e);
            std::process::exit(1);
        }
    };
    let devkitpro = Path::new(&devkitpro).canonicalize().unwrap();

    ninja.comment("Megaton runtime build.ninja");
    let common_flags = [
        "-march=armv8-a+crc+crypto",
        "-mtune=cortex-a57",
        "-mtp=soft",
        "-fPIC",
        "-fvisibility=hidden",
        "-g",
    ];
    let common_flags = common_flags.join(" ");

    ninja.variable("common_flags", &common_flags);

    let includes = [
        include, 
        src.join("exlaunch/source"),
        devkitpro.join("libnx/include"),
    ];

    let c_flags = [
        "-Wall",
        "-Werror",
        "-fdiagnostics-color=always",
        "-ffunction-sections",
        "-fdata-sections",
        "-O3",
        // temp. no longer needed when EXL is refactored
        "-DEXL_DEBUG",
        "-DEXL_USE_FAKEHEAP",
        "-DEXL_LOAD_KIND_ENUM=2",
        "-DEXL_LOAD_KIND=Module",
        "-DEXL_PROGRAM_ID=0x0100000000000000",
        "-DEXL_MODULE_NAME='\"test\"'",
        // ^^^
        &includes
            .into_iter()
            .map(|x| format!("-I{}", x.display()))
            .collect::<Vec<_>>()
            .join(" "),
    ];
    let c_flags = c_flags.join(" ");
    ninja.variable("c_flags", &c_flags);

    let cxx_flags = [
        "-std=gnu++20",
        "-fno-rtti",
        "-fno-exceptions",
        "-fno-asynchronous-unwind-tables",
        "-fno-unwind-tables",
    ];
    let cxx_flags = cxx_flags.join(" ");
    ninja.variable("cxx_flags", &cxx_flags);

    let as_flags = [
        format!("-x assembler-with-cpp {}", cxx_flags),
    ];
    let as_flags = as_flags.join(" ");
    ninja.variable("as_flags", &as_flags);
    let devkitpro = match devkitpro.join("devkitA64/bin").canonicalize() {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Cannot get devkitA64/bin: {}", e);
            std::process::exit(1);
        }
    };
    let devkitpro = {
        let mut x = devkitpro.display().to_string();
        if !x.ends_with('/') {
            x.push('/');
        }
        x.push_str("aarch64-none-elf-");
        x
    };

    ninja.variable("cc", format!("{devkitpro}gcc"));
    ninja.variable("cxx", format!("{devkitpro}g++"));

    let rule_as = ninja
        .rule(
            "as",
            "$cc -MD -MP -MF $out.d $as_flags $common_flags -c $in -o $out",
        )
        .depfile("$out.d")
        .deps_gcc()
        .description("AS $out");
    let rule_cc = ninja
        .rule(
            "cc",
            "$cc -MD -MP -MF $out.d $common_flags $c_flags -c $in -o $out",
        )
        .depfile("$out.d")
        .deps_gcc()
        .description("CC $out");
    let rule_cxx = ninja
        .rule(
            "cxx",
            "$cxx -MD -MP -MF $out.d $common_flags $c_flags $cxx_flags -c $in -o $out",
        )
        .depfile("$out.d")
        .deps_gcc()
        .description("CXX $out");
    match walk_directory(&src, &build, &rule_as, &rule_cc, &rule_cxx) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed walking directory: {}", e);
            std::process::exit(1);
        }
    };

    let generator = ninja
        .rule(
            "configure",
            "cargo --color always run --example configure -- $out",
        )
        .description("Configuring build.ninja")
        .generator();

    let mut args = std::env::args().skip(1);
    let build_ninja_path = args.next().unwrap();

    generator
        .build([ &build_ninja_path ])
        .with_implicit(["configure.rs"]);

    std::fs::write(build_ninja_path, ninja.to_string()).unwrap();
}

fn walk_directory(
    src: &Path,
    build: &Path,
    rule_as: &RuleRef,
    rule_cc: &RuleRef,
    rule_cxx: &RuleRef,
) -> std::io::Result<()> {
    let mut create_build = false;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_directory(&path, &build.join(path.file_name().unwrap()), rule_as, rule_cc, rule_cxx)?;
        } else {
            let src_path = path.display().to_string();
            let file_name = path.file_name().unwrap().to_string_lossy();
            let file_name = format!("{}.o", file_name);
            let build_path = build.join(file_name).display().to_string();
            if src_path.ends_with(".s") {
                rule_as.build([build_path]).with([src_path]);
                create_build = true;
            } else if src_path.ends_with(".c") {
                rule_cc.build([build_path]).with([src_path]);
                create_build = true;
            } else if src_path.ends_with(".cpp") {
                rule_cxx.build([build_path]).with([src_path]);
                create_build = true;
            }
        }
    }
    if create_build && !build.exists() {
        std::fs::create_dir_all(build)?;
    }

    Ok(())
}
