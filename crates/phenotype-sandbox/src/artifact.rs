// SPDX-License-Identifier: MIT OR Apache-2.0
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// An artifact produced by a sandbox execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artifact {
    /// SHA-256 of the stdout output.
    pub sha256: String,
    /// Optional signature of the artifact.
    pub signature: Option<String>,
    /// Optional provenance metadata.
    pub provenance: Option<String>,
    /// Optional SBOM reference.
    pub sbom: Option<String>,
}

/// Builder for constructing artifacts.
#[derive(Debug, Default)]
pub struct ArtifactBuilder {
    sha256: String,
    signature: Option<String>,
    provenance: Option<String>,
    sbom: Option<String>,
}

impl ArtifactBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the SHA-256 directly.
    pub fn sha256(mut self, sha256: impl Into<String>) -> Self {
        self.sha256 = sha256.into();
        self
    }

    /// Compute SHA-256 from raw bytes.
    pub fn sha256_from_bytes(mut self, bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        self.sha256 = hex::encode(hasher.finalize());
        self
    }

    /// Set an optional signature.
    pub fn signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    /// Set optional provenance.
    pub fn provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    /// Set an optional SBOM reference.
    pub fn sbom(mut self, sbom: impl Into<String>) -> Self {
        self.sbom = Some(sbom.into());
        self
    }

    /// Build the artifact.
    pub fn build(self) -> Artifact {
        Artifact {
            sha256: self.sha256,
            signature: self.signature,
            provenance: self.provenance,
            sbom: self.sbom,
        }
    }
}

impl Artifact {
    /// Create a builder for an artifact.
    pub fn builder() -> ArtifactBuilder {
        ArtifactBuilder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_sha256_from_bytes() {
        let data = b"hello world";
        let artifact = Artifact::builder().sha256_from_bytes(data).build();
        // SHA-256 of "hello world"
        assert_eq!(
            artifact.sha256,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn artifact_builder_full() {
        let artifact = Artifact::builder()
            .sha256("abc123")
            .signature("sig")
            .provenance("prov")
            .sbom("sbom")
            .build();

        assert_eq!(artifact.sha256, "abc123");
        assert_eq!(artifact.signature, Some("sig".to_string()));
        assert_eq!(artifact.provenance, Some("prov".to_string()));
        assert_eq!(artifact.sbom, Some("sbom".to_string()));
    }
}
