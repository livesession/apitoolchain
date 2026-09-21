//go:build wasip1

package clipboard

import "errors"

// wasip1 has no pbcopy/xclip to exec and no /dev/snarf. clipboard.go declares
// `Unsupported` precisely so callers can degrade; set it and fail explicitly.
func init() { Unsupported = true }

var errUnsupported = errors.New("clipboard: unsupported on wasip1")

func readAll() (string, error)   { return "", errUnsupported }
func writeAll(text string) error { return errUnsupported }
