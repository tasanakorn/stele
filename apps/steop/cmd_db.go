package main

import (
	"errors"
	"fmt"

	"github.com/tasanakorn/stele/apps/steop/internal/datadir"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

// RunDB dispatches `steop db <subcommand>`. Supported subcommands:
//   - init  — resolve path, open DB (creating + migrating as needed), print path.
//   - path  — print the resolved path without touching the file.
//
// Exported so main.go can wire it into the top-level dispatcher in Group D.
func RunDB(args []string) error {
	if len(args) == 0 {
		return errors.New("usage: steop db <init|path>")
	}
	switch args[0] {
	case "init":
		return runDBInit()
	case "path":
		return runDBPath()
	default:
		return fmt.Errorf("unknown db subcommand: %s", args[0])
	}
}

func runDBInit() error {
	path, err := datadir.DBPath()
	if err != nil {
		return err
	}
	db, err := store.Open(path)
	if err != nil {
		return err
	}
	if err := db.Close(); err != nil {
		return err
	}
	fmt.Println(path)
	return nil
}

func runDBPath() error {
	path, err := datadir.DBPath()
	if err != nil {
		return err
	}
	fmt.Println(path)
	return nil
}
