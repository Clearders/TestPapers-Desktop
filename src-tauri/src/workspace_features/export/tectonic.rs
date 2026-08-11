//! Locked-down adapter for a Tectonic binary and offline resource bundle shipped with the app.

use crate::workspace_features::hash::{sha256_file, Sha256Digest};
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) trait CompileControl: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

pub(crate) struct NoopCompileControl;

impl CompileControl for NoopCompileControl {
    fn is_cancelled(&self) -> bool {
        false
    }
}

impl CompileControl for crate::workspace_features::jobs::CancellationToken {
    fn is_cancelled(&self) -> bool {
        self.is_cancelled()
    }
}

pub(crate) trait TectonicRunner: Send + Sync {
    fn availability(&self) -> Result<(), TectonicError>;

    /// Compiles a generated TeX file already present directly inside `working_directory`.
    fn compile(
        &self,
        working_directory: &Path,
        tex_filename: &str,
        control: &dyn CompileControl,
    ) -> Result<PathBuf, TectonicError>;
}

#[derive(Clone, Debug)]
pub(crate) struct BundledTectonic {
    pub(crate) binary: PathBuf,
    pub(crate) resource_bundle: PathBuf,
    pub(crate) expected_binary_sha256: Option<Sha256Digest>,
    pub(crate) expected_bundle_sha256: Option<Sha256Digest>,
    pub(crate) timeout: Duration,
    pub(crate) max_pdf_bytes: u64,
}

impl BundledTectonic {
    pub(crate) fn new(binary: PathBuf, resource_bundle: PathBuf) -> Self {
        Self {
            binary,
            resource_bundle,
            expected_binary_sha256: None,
            expected_bundle_sha256: None,
            timeout: Duration::from_secs(60),
            max_pdf_bytes: 100 * 1024 * 1024,
        }
    }

    fn verify_file(
        path: &Path,
        expected: Option<Sha256Digest>,
        label: &'static str,
    ) -> Result<(), TectonicError> {
        if !path.is_file() {
            return Err(TectonicError::Unavailable(format!(
                "the bundled {label} is not installed"
            )));
        }
        if let Some(expected) = expected {
            let actual = sha256_file(path).map_err(|error| {
                TectonicError::Unavailable(format!("the bundled {label} cannot be read: {error}"))
            })?;
            if actual != expected {
                return Err(TectonicError::Unavailable(format!(
                    "the bundled {label} failed its checksum"
                )));
            }
        }
        Ok(())
    }
}

impl TectonicRunner for BundledTectonic {
    fn availability(&self) -> Result<(), TectonicError> {
        if self.expected_binary_sha256.is_none() || self.expected_bundle_sha256.is_none() {
            return Err(TectonicError::Unavailable(
                "release checksums for Tectonic and its offline bundle are not configured".into(),
            ));
        }
        Self::verify_file(
            &self.binary,
            self.expected_binary_sha256,
            "Tectonic executable",
        )?;
        Self::verify_file(
            &self.resource_bundle,
            self.expected_bundle_sha256,
            "Tectonic resource bundle",
        )
    }

    fn compile(
        &self,
        working_directory: &Path,
        tex_filename: &str,
        control: &dyn CompileControl,
    ) -> Result<PathBuf, TectonicError> {
        self.availability()?;
        validate_tex_filename(tex_filename)?;
        let work = working_directory.canonicalize().map_err(|error| {
            TectonicError::InvalidInput(format!("working directory is unavailable: {error}"))
        })?;
        if !work.is_dir() {
            return Err(TectonicError::InvalidInput(
                "working directory is not a directory".into(),
            ));
        }
        let tex_path = work.join(tex_filename);
        if !tex_path.is_file() {
            return Err(TectonicError::InvalidInput(
                "the generated TeX source is missing".into(),
            ));
        }
        let canonical_tex = tex_path.canonicalize().map_err(|error| {
            TectonicError::InvalidInput(format!("TeX source is unavailable: {error}"))
        })?;
        if canonical_tex.parent() != Some(work.as_path()) {
            return Err(TectonicError::InvalidInput(
                "TeX source must be directly inside the isolated working directory".into(),
            ));
        }
        if control.is_cancelled() {
            return Err(TectonicError::Cancelled);
        }

        let binary = self.binary.canonicalize().map_err(|error| {
            TectonicError::Unavailable(format!("Tectonic executable is unavailable: {error}"))
        })?;
        let bundle = self.resource_bundle.canonicalize().map_err(|error| {
            TectonicError::Unavailable(format!("Tectonic bundle is unavailable: {error}"))
        })?;
        let mut command = Command::new(binary);
        command
            .current_dir(&work)
            .env_clear()
            .env("TECTONIC_UNTRUSTED_MODE", "1")
            .arg("-X")
            .arg("compile")
            .arg("--untrusted")
            .arg("--only-cached")
            .arg("--keep-logs")
            .arg("--outdir")
            .arg(&work)
            .arg("--bundle")
            .arg(bundle)
            .arg(tex_filename)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command.spawn().map_err(|error| {
            TectonicError::Unavailable(format!("Tectonic could not start: {error}"))
        })?;
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            if control.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(TectonicError::Cancelled);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(TectonicError::TimedOut(self.timeout));
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(TectonicError::Failed(format!(
                        "could not observe Tectonic: {error}"
                    )));
                }
            }
        };
        if !status.success() {
            return Err(TectonicError::Failed(format!(
                "Tectonic exited with status {}",
                status
                    .code()
                    .map_or_else(|| "terminated".into(), |code| code.to_string())
            )));
        }
        let pdf_path = work.join(Path::new(tex_filename).with_extension("pdf"));
        let metadata = fs::metadata(&pdf_path)
            .map_err(|_| TectonicError::Failed("Tectonic produced no PDF".into()))?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > self.max_pdf_bytes {
            return Err(TectonicError::Failed(
                "Tectonic produced an empty or oversized PDF".into(),
            ));
        }
        let prefix = fs::read(&pdf_path)
            .map_err(|error| TectonicError::Failed(format!("PDF cannot be read: {error}")))?;
        if !prefix.starts_with(b"%PDF-") {
            return Err(TectonicError::Failed(
                "Tectonic output is not a valid PDF header".into(),
            ));
        }
        Ok(pdf_path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TectonicError {
    Unavailable(String),
    InvalidInput(String),
    Cancelled,
    TimedOut(Duration),
    Failed(String),
}

impl fmt::Display for TectonicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) => write!(formatter, "PDF export is unavailable: {message}"),
            Self::InvalidInput(message) => write!(formatter, "invalid PDF export input: {message}"),
            Self::Cancelled => formatter.write_str("PDF export was cancelled"),
            Self::TimedOut(timeout) => write!(formatter, "PDF export exceeded {timeout:?}"),
            Self::Failed(message) => write!(formatter, "PDF export failed: {message}"),
        }
    }
}

fn validate_tex_filename(filename: &str) -> Result<(), TectonicError> {
    let path = Path::new(filename);
    if filename.is_empty()
        || path.file_name() != Some(OsStr::new(filename))
        || path.extension() != Some(OsStr::new("tex"))
    {
        return Err(TectonicError::InvalidInput(
            "TeX filename must be a direct `.tex` basename".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_unbundled_sidecar_without_spawning_a_process() {
        let runner = BundledTectonic::new(
            PathBuf::from("definitely-missing-tectonic"),
            PathBuf::from("definitely-missing-bundle"),
        );
        assert!(matches!(
            runner.availability(),
            Err(TectonicError::Unavailable(_))
        ));
    }

    #[test]
    fn rejects_paths_as_tex_filenames() {
        assert!(validate_tex_filename("paper.tex").is_ok());
        assert!(validate_tex_filename("../paper.tex").is_err());
        assert!(validate_tex_filename("paper.txt").is_err());
    }
}
