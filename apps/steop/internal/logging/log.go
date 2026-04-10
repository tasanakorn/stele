package logging

import (
	"fmt"
	"os"
)

// Debugf writes to stderr only when STEOP_DEBUG=1 is set.
func Debugf(format string, args ...interface{}) {
	if os.Getenv("STEOP_DEBUG") != "1" {
		return
	}
	fmt.Fprintf(os.Stderr, "steop debug: "+format+"\n", args...)
}

// Errorf always writes to stderr.
func Errorf(format string, args ...interface{}) {
	fmt.Fprintf(os.Stderr, "steop error: "+format+"\n", args...)
}
