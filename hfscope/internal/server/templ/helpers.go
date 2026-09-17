// Package templ provides HTML rendering via github.com/a-h/templ.
//
// The package is intentionally split across several files to keep each
// template file short (long .templ files have been observed to confuse
// the templ parser, so we keep things below ~150 lines per file).
package templ

import (
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/KooshaPari/hfscope/internal/hfapi"
	"github.com/KooshaPari/hfscope/internal/views"
)
// formatNumber returns n with comma separators (1234567 -> "1,234,567").
// Returns "" for n == 0 so templates can decide how to render the empty case.
func formatNumber(n int) string {
	if n == 0 {
		return ""
	}
	negative := n < 0
	if negative {
		n = -n
	}
	s := strconv.Itoa(n)
	out := make([]byte, 0, len(s)+len(s)/3)
	for i, c := range []byte(s) {
		if i > 0 && (len(s)-i)%3 == 0 {
			out = append(out, ',')
		}
		out = append(out, c)
	}
	if negative {
		return "-" + string(out)
	}
	return string(out)
}

// trimShort returns s clipped to n bytes with an ellipsis suffix.
func trimShort(s string, n int) string {
	if len(s) <= n {
		return s
	}
	if n < 3 {
		return s[:n]
	}
	return s[:n-1] + "…"
}

// isActive reports whether a request path matches a nav item.
func isActive(path, want string) bool {
	if want == "/" {
		return path == "/"
	}
	return len(path) >= len(want) && path[:len(want)] == want
}

// modelURL returns the internal detail URL for a model id.
func modelURL(id string) string { return "/models/" + id }

// hfURL returns the absolute HuggingFace URL for a model id.
func hfURL(base, id string) string {
	if base == "" {
		return "https://huggingface.co/" + id
	}
	if base[len(base)-1] == '/' {
		return base + id
	}
	return base + "/" + id
}

// truncTags splits tags into a head and a "more" count.
func truncTags(tags []string, n int) (head []string, more int) {
	if len(tags) <= n {
		return tags, 0
	}
	return tags[:n], len(tags) - n
}

// relativeTime renders a short "3d ago" / "2mo ago" style string.
// Returns "" when t is zero.
func relativeTime(t time.Time) string {
	if t.IsZero() {
		return ""
	}
	d := time.Since(t)
	switch {
	case d < time.Minute:
		return "just now"
	case d < time.Hour:
		return fmt.Sprintf("%dm ago", int(d.Minutes()))
	case d < 24*time.Hour:
		return fmt.Sprintf("%dh ago", int(d.Hours()))
	case d < 30*24*time.Hour:
		return fmt.Sprintf("%dd ago", int(d.Hours()/24))
	case d < 365*24*time.Hour:
		return fmt.Sprintf("%dmo ago", int(d.Hours()/24/30))
	default:
		return fmt.Sprintf("%dy ago", int(d.Hours()/24/365))
	}
}

// truncateText returns the first n runes of s.
func truncateText(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "…"
}

// filterDisplayTags strips already-shown tag classes from the tag list so the
// pill row stays compact.
func filterDisplayTags(tags []string) []string {
	out := make([]string, 0, len(tags))
	for _, t := range tags {
		switch t {
		case "transformers", "safetensors", "pytorch", "endpoints_compatible",
			"text-generation-inference", "text-embeddings-inference",
			"model-index", "conversational":
			continue
		}
		if len(t) > 24 {
			continue
		}
		out = append(out, t)
	}
	return out
}

// tagJoinTitle returns the comma-separated display tag list for tooltip text.
func tagJoinTitle(tags []string) string {
	return strings.Join(filterDisplayTags(tags), ", ")
}

// boolStr renders a bool as "yes" / "no".
func boolStr(b bool) string {
	if b {
		return "yes"
	}
	return "no"
}

// intToStr renders an int as a string, "" when zero.
func intToStr(n int) string {
	if n == 0 {
		return ""
	}
	return strconv.Itoa(n)
}

// int64ToStr renders an int64 as a string, "" when zero. Used for param-size
// counts (which can exceed int32 on trillion-scale models).
func int64ToStr(n int64) string {
	if n == 0 {
		return ""
	}
	return strconv.FormatInt(n, 10)
}

// contains reports whether xs contains want.
func contains(xs []string, want string) bool {
	for _, x := range xs {
		if x == want {
			return true
		}
	}
	return false
}

// refDateMin is the lower bound for the created-range slider, expressed as
// days-since-epoch. Jan 1, 2015 covers the very first HF models and is well
// before the founding of the Hub.
func refDateMin() string {
	d, _ := time.Parse("2006-01-02", "2015-01-01")
	return int64ToStr(d.Unix() / 86400)
}

func refDateMax() string {
	return int64ToStr(time.Now().Unix() / 86400)
}

// paramMinFromURL, paramMaxFromURL, createdMinFromURL, createdMaxFromURL,
// downloadsFromURL, likesFromURL: tiny read-from-URL-state helpers used by
// refine.templ to hydrate the hidden inputs on initial page load. They read
// the same query string the parser would (so the URL is the single source
// of truth for both server-rendered and client-rendered state).
func paramMinFromURL(d views.PageData) string { return d.Params.Refine.ParamsMin }
func paramMaxFromURL(d views.PageData) string { return d.Params.Refine.ParamsMax }
func createdMinFromURL(d views.PageData) string {
	return d.Params.Refine.CreatedAfter
}
func createdMaxFromURL(d views.PageData) string {
	return d.Params.Refine.CreatedBefore
}
func downloadsFromURL(d views.PageData) string {
	return d.Params.Refine.MinDownloads
}
func likesFromURL(d views.PageData) string { return d.Params.Refine.MinLikes }

// formatCompact renders an integer count as a compact human string ("1.2K",
// "34M", "1.5B"). Used for the threshold chip next to refine inputs.
func formatCompact(n int64) string {
	switch {
	case n >= 1_000_000_000:
		return trimFloat(float64(n)/1_000_000_000) + "B"
	case n >= 1_000_000:
		return trimFloat(float64(n)/1_000_000) + "M"
	case n >= 1_000:
		return trimFloat(float64(n)/1_000) + "K"
	}
	return strconv.FormatInt(n, 10)
}

// trimFloat strips trailing zeros after the decimal point ("1.20" -> "1.2").
func trimFloat(f float64) string {
	s := strconv.FormatFloat(f, 'f', 2, 64)
	if i := strings.IndexByte(s, '.'); i >= 0 {
		s = strings.TrimRight(s[:i+1]+strings.TrimRight(s[i+1:], "0"), ".")
	}
	return s
}

// formatRefineChip renders the threshold chip next to a min-* input, e.g.
// "≥ 10K". Returns "" when the underlying threshold is empty/zero so the
// template can hide the chip.
func formatRefineChip(s string) string {
	s = strings.TrimSpace(s)
	if s == "" {
		return ""
	}
	n, err := strconv.ParseInt(s, 10, 64)
	if err != nil || n <= 0 {
		return ""
	}
	return "≥ " + formatCompact(n)
}

// parseInt64 is a small wrapper used by hfscope.js hydration (and tests).
func parseInt64(s string) int64 {
	n, _ := strconv.ParseInt(strings.TrimSpace(s), 10, 64)
	return n
}

// currentSortLabel returns the human label for the active sort value.
func currentSortLabel(v string) string {
	switch v {
	case "downloads", "":
		return "Most downloads"
	case "likes":
		return "Most likes"
	case "trendingScore":
		return "Trending"
	case "lastModified":
		return "Recently modified"
	}
	return v
}

// descriptionFor returns a sensible meta description for the current page.
func descriptionFor(d views.PageData) string {
	switch d.Title {
	case "Discover":
		return "HFScope: a better, controllable search client for the HuggingFace model hub."
	case "Search":
		return "Compose precise queries against the HuggingFace model hub with HFScope."
	case "About":
		return "About HFScope — how the controlled HuggingFace search client works."
	}
	if d.Title != "" {
		return "HuggingFace model " + d.Title + " via HFScope."
	}
	return "HFScope"
}

// idLabel is a (id, label) pair used by filter option lists.
type idLabel struct {
	ID    string
	Label string
}

// pipelineOptions returns the canonical list of pipeline tags we offer in
// the filter menu. The full taxonomy isn't returned by /api/models-tags-by-type
// for pipelines, so we hard-code the common ones HF exposes on the model
// search page.
func pipelineOptions(g hfapi.TagGroups) []idLabel {
	_ = g
	return []idLabel{
		{"text-generation", "Text Generation"},
		{"text2text-generation", "Text2Text Generation"},
		{"text-to-image", "Text-to-Image"},
		{"image-to-image", "Image-to-Image"},
		{"image-to-text", "Image-to-Text (captioning)"},
		{"image-classification", "Image Classification"},
		{"image-segmentation", "Image Segmentation"},
		{"image-feature-extraction", "Image Feature Extraction"},
		{"object-detection", "Object Detection"},
		{"zero-shot-image-classification", "Zero-shot Image Classification"},
		{"zero-shot-object-detection", "Zero-shot Object Detection"},
		{"automatic-speech-recognition", "Speech Recognition (ASR)"},
		{"text-to-speech", "Text-to-Speech"},
		{"audio-classification", "Audio Classification"},
		{"audio-to-audio", "Audio-to-Audio"},
		{"voice-activity-detection", "Voice Activity Detection"},
		{"text-classification", "Text Classification"},
		{"token-classification", "Token Classification (NER)"},
		{"translation", "Translation"},
		{"summarization", "Summarization"},
		{"conversational", "Conversational"},
		{"feature-extraction", "Feature Extraction (embeddings)"},
		{"sentence-similarity", "Sentence Similarity"},
		{"fill-mask", "Fill-Mask"},
		{"table-question-answering", "Table QA"},
		{"question-answering", "Extractive QA"},
		{"text-to-video", "Text-to-Video"},
		{"text-to-3d", "Text-to-3D"},
		{"image-to-video", "Image-to-Video"},
		{"video-classification", "Video Classification"},
		{"depth-estimation", "Depth Estimation"},
		{"reinforcement-learning", "Reinforcement Learning"},
		{"robotics", "Robotics"},
		{"any-to-any", "Any-to-Any"},
	}
}

func libraryOptions(g hfapi.TagGroups) []idLabel {
	out := []idLabel{}
	if g == nil {
		return out
	}
	for _, t := range g["library"] {
		out = append(out, idLabel{ID: t.ID, Label: t.Label})
	}
	return out
}

func languageOptions(g hfapi.TagGroups) []idLabel {
	out := []idLabel{}
	if g == nil {
		return out
	}
	for _, t := range g["language"] {
		out = append(out, idLabel{ID: t.ID, Label: t.Label})
	}
	return out
}

func licenseOptions(g hfapi.TagGroups) []idLabel {
	out := []idLabel{}
	if g == nil {
		return out
	}
	for _, t := range g["license"] {
		out = append(out, idLabel{ID: t.ID, Label: t.Label})
	}
	return out
}

// loadMoreValsJSON encodes the cursor and current params as JSON for hx-vals.
func loadMoreValsJSON(p hfapi.SearchParams, cursor string) string {
	parts := []string{}
	add := func(k, v string) {
		if v == "" {
			return
		}
		v = strings.ReplaceAll(v, `"`, `\"`)
		parts = append(parts, `"`+k+`":"`+v+`"`)
	}
	add("q", p.Search)
	add("author", p.Author)
	add("task", p.PipelineTag)
	add("library", p.Library)
	add("language", p.Language)
	add("license", p.License)
	add("sort", p.Sort)
	add("cursor", cursor)
	if p.Direction != 0 {
		parts = append(parts, `"direction":"`+intToStr(p.Direction)+`"`)
	}
	if p.Limit > 0 {
		parts = append(parts, `"limit":"`+intToStr(p.Limit)+`"`)
	}
	for _, f := range p.Filter {
		v := strings.ReplaceAll(f, `"`, `\"`)
		parts = append(parts, `"filter":"`+v+`"`)
	}
	out := "{"
	for i, p := range parts {
		if i > 0 {
			out += ","
		}
		out += p
	}
	out += "}"
	return out
}