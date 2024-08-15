# FunnyBones

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

- *Joints* are used to connect a specific end of one bone to a specific end of
another bone. Each joint can be assigned an angle which is applied as *forward
kinematics* to create the angle using the two associated bones.
- *Bones* are one-dimensional line segments that have a required length. Bones
  can have a *desired position* for the end of the bone positioned furthest from
  the skeleton root. If the desired position is set, it is applied as *inverse
  kinematics*. 
  
  In FunnyBones, bones come in two varieties: 

  - *Rigid* bones are a single line segment of a fixed length. An example of a
    rigid bone in a simple human skeleton might be a single bone representing
    the spine.
  - *Flexible* bones are two line segments of fixed lengths that bend and rotate
    automatically (ignoring the connecting joint's angle) to ensure that both
    leg segments are always the correct length. An example of a flexible bone in
    a simple human skeleton might be a leg or an arm.

A `Skeleton` is a collection of joints and bones. The first bone pushed is
considered the root bone. When solving for updated positions, the algorithm
starts by evaluating all joints connected to both ends of the root bone and
continues until all reachable bones have been evaluated. The algorithm is
single-pass and produces stable results.