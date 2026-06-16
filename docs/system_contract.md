# System Core Contract — Traxes

## 1. Deterministic Evaluation Guarantee

Identical inputs + identical policy version → identical output decision.

## 2. Policy Binding Guarantee

Every decision artifact MUST include immutable policy hash and version reference.

## 3. Replay Guarantee

Every artifact can be replayed to reproduce decision outcome under identical conditions.

## 4. Fail-Closed Guarantee

On error, missing policy, or invalid input → system returns DENY.

## 5. No Side Effects Guarantee

Traxes does not execute actions. It only evaluates and returns decisions.

## 6. Non-Goals

- Not a full observability system
- Not a security enforcement boundary
- Not responsible for upstream data correctness
- Not an application logic engine

## 7. Failure Modes

- missing policy → DENY
- invalid input → DENY
- runtime error → DENY

## Purpose

This contract defines the trust boundary for deterministic execution gating in automated systems.
