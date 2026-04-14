package store

import "testing"

func TestParseID_Valid(t *testing.T) {
	cases := []struct {
		in   string
		want Identity
	}{
		{
			in:   "vm-02:/home/tas/stele",
			want: Identity{Host: "vm-02", ProjectDir: "/home/tas/stele"},
		},
		{
			in:   "vm-02:/home/tas/stele:a1b2c3d4-5678-4abc-9def-0123456789ab",
			want: Identity{Host: "vm-02", ProjectDir: "/home/tas/stele", SessionID: "a1b2c3d4-5678-4abc-9def-0123456789ab"},
		},
		{
			in:   "laptop:/Users/tas/work:USER",
			want: Identity{Host: "laptop", ProjectDir: "/Users/tas/work", SessionID: "USER"},
		},
		{
			// project_dir containing colons is allowed as long as the final
			// tail is USER or a canonical UUID.
			in:   "h:C:/windows/path:USER",
			want: Identity{Host: "h", ProjectDir: "C:/windows/path", SessionID: "USER"},
		},
	}
	for _, c := range cases {
		got, err := ParseID(c.in)
		if err != nil {
			t.Errorf("ParseID(%q) err: %v", c.in, err)
			continue
		}
		if got != c.want {
			t.Errorf("ParseID(%q) = %+v, want %+v", c.in, got, c.want)
		}
	}
}

func TestParseID_Invalid(t *testing.T) {
	bad := []string{
		"",
		"no-colon",
		":/missing-host",
		"host:",
		"host:proj:notuuid",
		"host:proj:user",   // lowercase must be rejected
		"host:proj:USER/x", // extra garbage
		"host:proj:a1b2c3d4-5678-4abc-9def-0123456789",   // short uuid
		"host:proj:A1B2C3D4-5678-4abc-9def-0123456789ab", // uppercase uuid
	}
	for _, s := range bad {
		if _, err := ParseID(s); err == nil {
			t.Errorf("ParseID(%q) should fail", s)
		}
	}
}

func TestIsProject(t *testing.T) {
	id, _ := ParseID("h:/p")
	if !id.IsProject() {
		t.Fatal("expected project-level id")
	}
	id, _ = ParseID("h:/p:USER")
	if id.IsProject() {
		t.Fatal("expected session-level id")
	}
}
