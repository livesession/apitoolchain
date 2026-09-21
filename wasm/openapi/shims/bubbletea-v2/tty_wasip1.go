//go:build wasip1

package tea

// wasip1 has no controlling terminal, no termios and no POSIX signals.
// Upstream splits these four symbols across tty_unix.go / tty_windows.go and
// signals_unix.go / signals_windows.go; wasip1 matches neither tag set, so the
// package does not compile without this file.
//
// The bodies mirror the WINDOWS variants, which are already the "no SIGWINCH"
// degenerate case — except initInput, which returns an error instead of nil.
// That difference is deliberate: with a no-op initInput, `openapi spec explore`
// starts, renders once, and then blocks forever on an input reader that can
// never produce a key. An error makes p.Run() fail cleanly and the command exit
// non-zero with a message. A clear failure beats a hang.

import "errors"

// ErrNoTTY is returned by initInput: there is no terminal to put into raw mode.
var ErrNoTTY = errors.New("bubbletea: interactive input is unavailable on wasip1 (no TTY)")

func (p *Program) initInput() (err error) { return ErrNoTTY }

const suspendSupported = false

func suspendProcess() {}

// listenForResize is not available on wasip1: there is no SIGWINCH.
func (p *Program) listenForResize(done chan struct{}) { close(done) }
