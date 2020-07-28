# RustOS
This is an operating system for a Raspberry Pi written in Rust based on the
Georgia Tech class CS-3210, which was itself based on the Stanford class
CS140e. I originally started working on the CS140e version of the class then
moved all my code into this repository and updated everything to work with the
changes made by the Georgia Tech Staff/TAs. The purpose of this was to both learn
the Rust language and to better understand low level programming concepts.

## Setup and Build
Setup and build are done according to instructions at https://tc.gts3.org/cs3210/2020/spring/lab/tools.html

# CS3210 Lab assignments

This repository contains lab assignments for Georgia Tech CS3210 "Design of Operating Systems".
The latest course material is available [here](https://tc.gts3.org/cs3210/2020/spring/index.html).

## Who should take CS3210?

- Anyone wants to work on challenges in operating systems
- Anyone cares about what's going on under the hood
- Anyone has to build high-performance systems (e.g., Cloud, Trading)
- Anyone wants to build embedded/IoT firmware (e.g., Robot)
- Anyone needs to diagnose bugs or security problems

## Why Rust?

Historically, C has been mainly used for OS development because of its portability,
minimal runtime, direct hardware/memory access, and (decent) usability.
Rust provides all of these features with addition of memory safety guarantee,
strong type system, and modern language abstractions
which help programmers to make less mistakes when writing code.

## Acknowledgement

We built our labs based on the materials originally developed for
[CS140e: An Experimental Course on Operating Systems](https://cs140e.sergio.bz/)
by [Sergio Benitez](https://sergio.bz/).
We have ported it to use newer toolchains such as Rust 2018 edition,
`cargo-xbuild` (instead of `xargo`), and `no_std` Rust with a minimal shim library
(instead of custom built std).
We’ve also developed it further to include topics such as virtual memory management, multicore scheduling, mutex designing, and implementing a networking stack.
