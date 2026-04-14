// TODO: factor into internal/ packages mirroring Rust crate split once spike works
package main

import (
	"flag"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/eclipse-zenoh/zenoh-go/zenoh"
)

func buildConfig(connectEndpoints []string) (zenoh.Config, error) {
	cfg := zenoh.NewConfigDefault()

	if err := cfg.InsertJson5(zenoh.ConfigModeKey, "'peer'"); err != nil {
		return cfg, fmt.Errorf("set mode: %w", err)
	}
	if err := cfg.InsertJson5(zenoh.ConfigMulticastScoutingKey, "false"); err != nil {
		return cfg, fmt.Errorf("disable multicast: %w", err)
	}
	if len(connectEndpoints) > 0 {
		var buf strings.Builder
		buf.WriteString("[")
		for i, ep := range connectEndpoints {
			buf.WriteString("'")
			buf.WriteString(ep)
			buf.WriteString("'")
			if i+1 < len(connectEndpoints) {
				buf.WriteString(",")
			}
		}
		buf.WriteString("]")
		if err := cfg.InsertJson5(zenoh.ConfigConnectKey, buf.String()); err != nil {
			return cfg, fmt.Errorf("set connect endpoints: %w", err)
		}
	}
	return cfg, nil
}

func cmdGet(keyStr string, connectEndpoints []string, timeoutMs uint64) error {
	cfg, err := buildConfig(connectEndpoints)
	if err != nil {
		return err
	}

	session, err := zenoh.Open(cfg, nil)
	if err != nil {
		return fmt.Errorf("open session: %w", err)
	}
	defer session.Drop()

	keyExpr, err := zenoh.NewKeyExpr(keyStr)
	if err != nil {
		return fmt.Errorf("invalid key expression %q: %w", keyStr, err)
	}

	opts := zenoh.GetOptions{}
	opts.TimeoutMs = timeoutMs

	replies, err := session.Get(keyExpr, "", zenoh.NewFifoChannel[zenoh.Reply](16), &opts)
	if err != nil {
		return fmt.Errorf("get: %w", err)
	}

	for reply := range replies {
		if reply.IsOk() {
			sample := reply.Ok().Unwrap()
			fmt.Printf("%s\n", sample.Payload())
		} else {
			fmt.Fprintln(os.Stderr, "received error reply")
		}
	}
	return nil
}

func cmdPub(keyStr string, message string, connectEndpoints []string) error {
	cfg, err := buildConfig(connectEndpoints)
	if err != nil {
		return err
	}

	session, err := zenoh.Open(cfg, nil)
	if err != nil {
		return fmt.Errorf("open session: %w", err)
	}
	defer session.Drop()

	keyExpr, err := zenoh.NewKeyExpr(keyStr)
	if err != nil {
		return fmt.Errorf("invalid key expression %q: %w", keyStr, err)
	}

	pub, err := session.DeclarePublisher(keyExpr, nil)
	if err != nil {
		return fmt.Errorf("declare publisher: %w", err)
	}
	defer pub.Drop()

	if err := pub.Put(zenoh.NewZBytesFromString(message), &zenoh.PublisherPutOptions{}); err != nil {
		return fmt.Errorf("put: %w", err)
	}

	// brief pause to allow propagation before session drops
	time.Sleep(100 * time.Millisecond)
	return nil
}

func main() {
	zenoh.InitLoggerFromEnvOr("error")

	getCmd := flag.NewFlagSet("get", flag.ExitOnError)
	getConnect := getCmd.String("connect", "", "Endpoint to connect to (e.g. tcp/127.0.0.1:7447)")
	getTimeout := getCmd.Uint64("timeout-ms", 3000, "Query timeout in milliseconds")

	pubCmd := flag.NewFlagSet("pub", flag.ExitOnError)
	pubConnect := pubCmd.String("connect", "", "Endpoint to connect to")

	if len(os.Args) < 2 {
		fmt.Fprintf(os.Stderr, "usage: stylos <get|pub> [flags] <KEY> [MSG]\n")
		os.Exit(1)
	}

	switch os.Args[1] {
	case "get":
		getCmd.Parse(os.Args[2:])
		args := getCmd.Args()
		if len(args) < 1 {
			fmt.Fprintln(os.Stderr, "usage: stylos get [--connect ENDPOINT] [--timeout-ms N] <KEY>")
			os.Exit(1)
		}
		var endpoints []string
		if *getConnect != "" {
			endpoints = []string{*getConnect}
		}
		if err := cmdGet(args[0], endpoints, *getTimeout); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}

	case "pub":
		pubCmd.Parse(os.Args[2:])
		args := pubCmd.Args()
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: stylos pub [--connect ENDPOINT] <KEY> <MSG>")
			os.Exit(1)
		}
		var endpoints []string
		if *pubConnect != "" {
			endpoints = []string{*pubConnect}
		}
		if err := cmdPub(args[0], args[1], endpoints); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}

	default:
		fmt.Fprintf(os.Stderr, "unknown subcommand: %s\n", os.Args[1])
		os.Exit(1)
	}
}
