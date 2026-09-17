// Package views holds the data types shared between the HTTP server and the
// templ rendering package. Keeping these here breaks the import cycle
// between the server and templ packages.
package views

import (
	"fmt"
	"time"

	"github.com/KooshaPari/hfscope/internal/hfapi"
)

// PageData carries everything the server hands to a template render.
//
// All fields are optional; each handler populates only what its template needs.
// Boolean "ShowX" toggles let the layout hide chrome that doesn't belong on
// page types like /feed (RSS) and /results/compare (columnar).
type PageData struct {
	Title         string
	Description   string
	BaseURL       string
	HFBase        string
	CSPNonce      string
	ActiveNav     string // "home" | "search" | "results" | "compare" | "model"
	ShowSidebar   bool   // hide sidebar on /results/compare and /feed
	ShowSearchBox bool   // hide search input on compact views

	// Model-detail page (single column)
	Path        string
	GeneratedAt time.Time
	Model       hfapi.Model
	Summary     hfapi.Summary
	ModelDetail hfapi.ModelDetail
	DetailErr   string

	// Results page
	Params hfapi.SearchParams
	Result hfapi.ListResult
	Tags   hfapi.TagGroups

	// Card teaser enrichment (Tier 2.1)
	Teasers map[string]string

	// Compare page (Tier 2.3)
	CompareIDs  []string
	CompareRows []CompareColumn

	// Permalinks
	PermaQuery string

	// Sort dropdown
	Sorts []SortOption

	// Env (controls nav, badges, etc.)
	FeedHref     string

	IsProd bool
}

// SortOption is one entry in the Sort dropdown on /results.
type SortOption struct {
	Value     string
	Label     string
	Direction int // -1 = descending, 1 = ascending; 0 = server default
	Selected  bool
}

// CompareColumn is one model in a side-by-side compare table.
type CompareColumn struct {
	ID         string
	Model      hfapi.Model
	Summary    hfapi.Summary
	Detail     hfapi.ModelDetail
	FilesByCat map[hfapi.FileCategory]int
	Error      string
}

// ErrorData is rendered when the server can't talk to HuggingFace.
type ErrorData struct {
	Title   string
	Message string
}

// FormatBytes is a tiny helper so views can format numeric counts that
// templates may need without importing hfapi directly.
func FormatBytes(n int64) string {
	return fmt.Sprintf("%d B", n)
}
