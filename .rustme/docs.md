<!-- This file is generated by `rustme`. Ensure you're editing the source in the .rustme/ directory --!>
<!-- markdownlint-disable first-line-h1 -->

![FunnyBones is considered experimental and unsupported](https://img.shields.io/badge/status-experimental-purple)
[![crate version](https://img.shields.io/crates/v/muse.svg)](https://crates.io/crates/funnybones)
[![Documentation for `$ref-name$`](https://img.shields.io/badge/docs-$ref-name$-informational)]($docs$)

A simple 2D kinematics library for Rust

## Motivation and Goals

When looking at the libraries that support inverse kinematics in Rust in 2024,
there are several fairly mature solutions that focus on 3D and robotics. For
someone interested in 2D only, a lot of these libraries seemed like overkill for
basic 2D games.

This library implements a simplified forward and inverse kinematics model that
only uses basic trigonometry and can be solved in one pass across the bone
structure with no smoothing algorithms necessary. The initial implementation of
this library was under 600 lines of code with no required dependencies.

## How FunnyBones works

FunnyBones has two main concepts: joints and bones.

- [Joints][joint] are used to connect a specific end of one bone to a specific
  end of another bone. Each joint can be assigned an angle which is applied as
  *forward kinematics* to create the angle using the two associated bones.
- [Bones][bone] are one-dimensional line segments that have a required length.
  Bones can have a *desired position* for the end of the bone positioned
  furthest from the skeleton root. If the desired position is set, it is applied
  as *inverse kinematics*. 
  
  In FunnyBones, bones come in two varieties: 

  - *Rigid* bones are a single line segment of a fixed length. An example of a
    rigid bone in a simple human skeleton might be a single bone representing
    the spine.
  - *Flexible* bones are two line segments of fixed lengths that bend and rotate
    automatically (ignoring the connecting joint's angle) to ensure that both
    leg segments are always the correct length. An example of a flexible bone in
    a simple human skeleton might be a leg or an arm.

A [`Skeleton`][skeleton] is a collection of joints and bones. The first bone
pushed is considered the root bone. When solving for updated positions, the
algorithm starts by evaluating all joints connected to both ends of the root
bone and continues until all reachable bones have been evaluated. The algorithm
is single-pass and produces stable results.

## FunnyBones in Action

The [`skeleton` example][skeleton-example] in the repository uses
[Cushy](https://github.com/khonsulabs/cushy) to draw and allow changing various
settings of a basic humanoid skeleton:

```rust,ignore
$../examples/skeleton.rs:readme$
```

<video src="https://raw.githubusercontent.com/khonsulabs/FunnyBones/gh-pages/20240815-1619-47.3700715.mp4" controls="true" autoplay="true" loop="true" muted="true"></video>

The example draws a small white circle where a desired location for a joint is.
FunnyBones ensures that all bones remain their original lengths while solving
the kinematics.

[skeleton]: $skeleton$
[joint]: $joint$
[bone]: $bone$
[skeleton-example]: https://github.com/khonsulabs/FunnyBones/tree/$ref-name$/examples/skeleton.rs