# Verus Codebases

A curated collection of Verus projects for analysis and verification research.

**Total Projects:** 20

## Verus Core Projects

| Project | Description | GitHub |
|---------|-------------|--------|
| **verus** | The Verus verification tool for Rust | https://github.com/verus-lang/verus |
| **verus-analyzer** | Language server and IDE support for Verus | https://github.com/verus-lang/verus-analyzer |
| **vostd** | Verus standard library extensions (Asterinas) | https://github.com/asterinas/vostd |

## Verified Systems

| Project | Description | GitHub |
|---------|-------------|--------|
| **atmosphere** | Fully verified microkernel based on L4 design principles | https://github.com/mars-research/atmosphere |
| **verified-ironkv** | Verified IronFleet key-value store in Verus | https://github.com/verus-lang/verified-ironkv |
| **verified-nrkernel** | Verified NR kernel implementation | https://github.com/matthias-brun/verified-nrkernel |
| **verified-paging-for-x86-64-in-rust** | Verified x86-64 page tables (Master's Thesis) | https://github.com/matthias-brun/verified-paging-for-x86-64-in-rust |
| **verified-node-replication** | Library for linearizable, NUMA-aware concurrent data structures | https://github.com/verus-lang/verified-node-replication |
| **verified-memory-allocator** | Verified memory allocator implementation | https://github.com/verus-lang/verified-memory-allocator |
| **verified-storage** | Verified storage systems (Microsoft Research) | https://github.com/microsoft/verified-storage |
| **verismo** | Verified security monitor for confidential VMs (Microsoft) | https://github.com/microsoft/verismo |
| **CortenMM-Artifact** | Verified memory management artifact | https://github.com/TELOS-syslab/CortenMM-Artifact |

## Verification Tools & Frameworks

| Project | Description | GitHub |
|---------|-------------|--------|
| **anvil** | Verification framework for Kubernetes controllers | https://github.com/anvil-verifier/anvil |
| **verdict** | Verification tooling and infrastructure | https://github.com/secure-foundations/verdict |
| **leaf** | Verified data structure library | https://github.com/secure-foundations/leaf |
| **vest** | Verified serialization/deserialization framework | https://github.com/secure-foundations/vest |

## Cryptographic Protocols

| Project | Description | GitHub |
|---------|-------------|--------|
| **owl** | Compiler for cryptographic protocols (OwlC) | https://github.com/secure-foundations/owl |

## Research & Benchmarks

| Project | Description | GitHub |
|---------|-------------|--------|
| **alphaverus** | LLM training for Verus code generation | https://github.com/cmu-l3/alphaverus |
| **human-eval-verus** | Verus examples for evaluation benchmarks | https://github.com/secure-foundations/human-eval-verus |
| **APAS-VERUS** | Algorithms & Parallel Systems in Verus | https://github.com/briangmilnes/APAS-VERUS |

## Organizations Represented

| Organization | Projects |
|--------------|----------|
| **verus-lang** | verus, verus-analyzer, verified-ironkv, verified-memory-allocator, verified-node-replication |
| **secure-foundations** | verdict, leaf, vest, owl, human-eval-verus |
| **microsoft** | verified-storage, verismo |
| **matthias-brun** | verified-nrkernel, verified-paging-for-x86-64-in-rust |
| **mars-research** | atmosphere |
| **asterinas** | vostd |
| **anvil-verifier** | anvil |
| **cmu-l3** | alphaverus |
| **TELOS-syslab** | CortenMM-Artifact |
| **briangmilnes** | APAS-VERUS |

## Quick Clone All

```bash
mkdir -p ~/projects/VerusCodebases && cd ~/projects/VerusCodebases

# Core
git clone https://github.com/verus-lang/verus.git
git clone https://github.com/verus-lang/verus-analyzer.git
git clone https://github.com/asterinas/vostd.git

# Systems
git clone https://github.com/mars-research/atmosphere.git
git clone https://github.com/verus-lang/verified-ironkv.git
git clone https://github.com/matthias-brun/verified-nrkernel.git
git clone https://github.com/matthias-brun/verified-paging-for-x86-64-in-rust.git
git clone https://github.com/verus-lang/verified-node-replication.git
git clone https://github.com/verus-lang/verified-memory-allocator.git
git clone https://github.com/microsoft/verified-storage.git
git clone https://github.com/microsoft/verismo.git
git clone https://github.com/TELOS-syslab/CortenMM-Artifact.git

# Tools
git clone https://github.com/anvil-verifier/anvil.git
git clone https://github.com/secure-foundations/verdict.git
git clone https://github.com/secure-foundations/leaf.git
git clone https://github.com/secure-foundations/vest.git

# Crypto
git clone https://github.com/secure-foundations/owl.git

# Research
git clone https://github.com/cmu-l3/alphaverus.git
git clone https://github.com/secure-foundations/human-eval-verus.git
git clone https://github.com/briangmilnes/APAS-VERUS.git
```

## Statistics

Run `veracity-analyze-modules-mir -M ~/projects/VerusCodebases` to generate usage statistics for vstd modules, types, and methods across all codebases.

