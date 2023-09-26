# Signatures capabilities of Lanzaboote

Currently, lanzaboote can perform signatures of PE binaries based on local keypairs present on disk.

## Overview

Lanzaboote is able to abstract the concept of a signer via a trait called `LanzabooteSigner`, such a trait can perform any operation to:

- build and sign a stub
- sign a certain Nix store path
- sign and copy from a certain location to another: this can be automatically derived from the others implementations, but sometimes, you can provide a simpler implementation.
- get an opaque public key represented as bytes: this is used for content addressing our signed bootables on the ESP partition
- verify a PE binary for its signatures: the trait implementation should encode the various trusted keys parameters itself
- verify a path: this can be automatically derived from the previous function but can be simpler in certain cases (when shelling to external binaries)

## Local keypairs

Storing your signature keys in the disk in some location we can read is the most trivial signature capability
we can offer.

You are responsible for securing them and ensuring they are not accessible by an attacker.

Signature happens via `sbsign` which will copy your input inside a temporary directory, sign it, read it and offers it to you again.

In the future, we may remove `sbsign` dependency to perform signature in-memory without any temporary directory.

## How to add a signature scheme?

To add a new signature scheme, you need two things:

- a minimal implementation of `LanzabooteSigner`
- passing this minimal implementation to any tool of your choice: not all tools have to support your new signature scheme

A good first target for the tool is the systemd one as it is the most supported and featureful.
