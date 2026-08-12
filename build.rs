use std::env;
use std::process::Command;

fn main() {
	println!("cargo:rerun-if-env-changed=RUSTFLAGS");
	println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");

	let has_bmi2 = env::var("CARGO_CFG_TARGET_FEATURE")
		.map(|features| features.split(',').any(|f| f == "bmi2"))
		.unwrap_or(false);

	if !has_bmi2 { return; }

	let slow_bmi2_cpus = ["excavator", "znver1", "znver2", "geode", "k6-2"];

	if let Some(mut target_cpu) = get_target_cpu() {
		if target_cpu == "native" && let Some(actual_cpu) = get_native_cpu() {
			target_cpu = actual_cpu;
		}

		if slow_bmi2_cpus.contains(&target_cpu.as_str()) { return; }
	}

	println!("cargo:rustc-cfg=fast_bmi2");
}

fn get_target_cpu() -> Option<String> {
	let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS")
		.map(|flags| flags.split('\x1f').map(String::from).collect::<Vec<_>>())
		.or_else(|_| {
			env::var("RUSTFLAGS").map(|flags| {
				flags
					.split_whitespace()
					.map(String::from)
					.collect::<Vec<_>>()
			})
		})
		.unwrap_or_default();

	for chunk in rustflags.windows(2) {
		if chunk[0] == "-C" && chunk[1].starts_with("target-cpu=") {
			return Some(
				chunk[1]
					.trim_start_matches("target-cpu=")
					.to_lowercase(),
			);
		}
	}

	for flag in &rustflags {
		if flag.starts_with("-Ctarget-cpu=") {
			return Some(
				flag.trim_start_matches("-Ctarget-cpu=")
					.to_lowercase(),
			);
		}
	}

	None
}

fn get_native_cpu() -> Option<String> {
	let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
	let mut cmd = Command::new(rustc);

	cmd.arg("--print").arg("target-cpus");

	if let Ok(target) = env::var("TARGET") {
		cmd.arg("--target").arg(target);
	}

	let output = cmd.output().ok()?;
	let stdout = String::from_utf8(output.stdout).ok()?;

	for line in stdout.lines() {
		let line = line.trim();
		if line.starts_with("native") 
			&& let Some(start_idx) = line.find("(currently ") 
		{
			let name_start = start_idx + "(currently ".len();
			if let Some(end_offset) = line[name_start..].find(')') {
				return Some(line[name_start..name_start + end_offset].to_string());
			}
		}
		
	}

	None
}