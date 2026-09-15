package hfapi

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"
)

// jsonMarshalIndent is a tiny shim so the call site reads cleanly.
func jsonMarshalIndent(v any, _, indent string) ([]byte, error) {
	return json.MarshalIndent(v, "", indent)
}

// FileEntry, FileCategory, and Category* constants live in client.go
// (canonical location for the data type). This file only holds the helpers
// that operate on those types.

// CategorizeFile returns the FileCategory for a given filename. Used by
// the file-grid on the model detail page to group siblings.
func CategorizeFile(name string) FileCategory {
	lower := strings.ToLower(name)
	switch {
	case strings.HasSuffix(lower, ".safetensors"):
		return CategorySafetensors
	case strings.HasSuffix(lower, ".gguf"):
		return CategoryGGUF
	case strings.HasSuffix(lower, ".pt"), strings.HasSuffix(lower, ".pth"):
		return CategoryPT
	case strings.HasSuffix(lower, ".bin"):
		return CategoryBin
	case strings.HasSuffix(lower, ".model"), strings.HasSuffix(lower, ".tiktoken"):
		return CategoryTokenizer
	case strings.HasSuffix(lower, ".json"), strings.HasSuffix(lower, ".jsonl"):
		return CategoryJSON
	case strings.HasSuffix(lower, ".md"), strings.HasSuffix(lower, ".markdown"):
		return CategoryMarkdown
	case strings.HasSuffix(lower, ".txt"), strings.HasSuffix(lower, ".yaml"),
		strings.HasSuffix(lower, ".yml"), strings.HasSuffix(lower, ".toml"):
		return CategoryConfig
	default:
		return CategoryOther
	}
}

// FileGroup bundles siblings of one category with category-level totals
// for the file-grid template.
type FileGroup struct {
	Category  FileCategory
	Label     string
	Files     []FileEntry
	TotalSize int64
}

// GroupFiles categorizes a list of FileEntry by extension and returns one
// FileGroup per category, ordered by the most-relevant first.
func GroupFiles(files []FileEntry) []FileGroup {
	if len(files) == 0 {
		return nil
	}
	buckets := make(map[FileCategory][]FileEntry, 9)
	for _, f := range files {
		if isHiddenOrSkippable(f.RFilename) {
			continue
		}
		cat := CategorizeFile(f.RFilename)
		buckets[cat] = append(buckets[cat], f)
	}
	if len(buckets) == 0 {
		return nil
	}
	order := []FileCategory{
		CategorySafetensors, CategoryGGUF, CategoryPT, CategoryBin,
		CategoryTokenizer, CategoryJSON, CategoryConfig, CategoryMarkdown, CategoryOther,
	}
	out := make([]FileGroup, 0, len(buckets))
	for _, cat := range order {
		filesInCat, ok := buckets[cat]
		if !ok || len(filesInCat) == 0 {
			continue
		}
		sort.Slice(filesInCat, func(i, j int) bool {
			return filesInCat[i].RFilename < filesInCat[j].RFilename
		})
		var total int64
		for _, f := range filesInCat {
			total += f.Size
		}
		out = append(out, FileGroup{
			Category:  cat,
			Label:     fileCategoryLabel(cat),
			Files:     filesInCat,
			TotalSize: total,
		})
	}
	return out
}

func fileCategoryLabel(cat FileCategory) string {
	switch cat {
	case CategorySafetensors:
		return "Safetensors"
	case CategoryGGUF:
		return "GGUF"
	case CategoryBin:
		return "Bin (legacy)"
	case CategoryPT:
		return "PyTorch"
	case CategoryJSON:
		return "JSON / metadata"
	case CategoryTokenizer:
		return "Tokenizer"
	case CategoryMarkdown:
		return "Markdown"
	case CategoryConfig:
		return "Config / docs"
	case CategoryOther:
		return "Other"
	}
	return string(cat)
}

func isHiddenOrSkippable(name string) bool {
	lower := strings.ToLower(name)
	if strings.HasPrefix(lower, ".") {
		// Allow .md / .markdown since some models use dotfile-style READMEs
		// but exclude .gitignore, .gitattributes, .git-blame-ignore-revs, etc.
		if strings.HasSuffix(lower, ".md") || strings.HasSuffix(lower, ".markdown") {
			return false
		}
		return true
	}
	base := lower
	if i := strings.LastIndex(lower, "/"); i >= 0 {
		base = lower[i+1:]
	}
	switch base {
	case "license", "notice", "readme":
		return true
	}
	return false
}

// FormatBytes renders a byte count in a human-friendly form. 0 returns "—".
func FormatBytes(n int64) string {
	if n <= 0 {
		return "—"
	}
	const k = 1024
	switch {
	case n < k:
		return fmt.Sprintf("%d B", n)
	case n < k*k:
		return fmt.Sprintf("%.1f KB", float64(n)/k)
	case n < k*k*k:
		return fmt.Sprintf("%.1f MB", float64(n)/(k*k))
	case n < k*k*k*k:
		return fmt.Sprintf("%.2f GB", float64(n)/(k*k*k))
	default:
		return fmt.Sprintf("%.2f TB", float64(n)/(k*k*k*k))
	}
}

// SumBytes sums the Size field of a file list. Returns 0 when the list is
// empty or all sizes are unknown.
func SumBytes(files []FileEntry) int64 {
	var total int64
	for _, f := range files {
		total += f.Size
	}
	return total
}

// PrettyJSON marshals v as a 2-space-indented JSON string. Returns "" on
// marshal error. Used to render the config block in the model detail page.
func PrettyJSON(v any) string {
	if v == nil {
		return ""
	}
	// Re-marshal via encoding/json for stable formatting
	b, err := jsonMarshalIndent(v, "", "  ")
	if err != nil {
		return ""
	}
	return string(b)
}

// ----------------------------------------------------------------------------
// Tag-derived display helpers
// ----------------------------------------------------------------------------

// copyleft lists license IDs we want to flag as copyleft in the UI.
var copyleft = map[string]bool{
	"gpl": true, "gpl-2.0": true, "gpl-3.0": true,
	"agpl-3.0": true, "lgpl": true, "lgpl-2.1": true, "lgpl-3.0": true,
}

// licenseOf returns the first license:* tag value, or "" if none.
func licenseOf(tags []string) string {
	for _, t := range tags {
		if strings.HasPrefix(t, "license:") {
			return strings.TrimPrefix(t, "license:")
		}
	}
	return ""
}

// regionOf returns the first region:* tag value, or "" if none.
func regionOf(tags []string) string {
	for _, t := range tags {
		if strings.HasPrefix(t, "region:") {
			return strings.TrimPrefix(t, "region:")
		}
	}
	return ""
}

// gatedString normalises HF's gated field to "" / "auto" / "manual" / "true".
func gatedString(v any) string {
	switch x := v.(type) {
	case nil:
		return ""
	case bool:
		if x {
			return "true"
		}
		return ""
	case string:
		return x
	}
	return ""
}

// gatedIsOn returns true when the model is gated in any form.
func gatedIsOn(v any) bool {
	switch x := v.(type) {
	case bool:
		return x
	case string:
		return x == "auto" || x == "manual" || x == "true"
	}
	return false
}

// GatedLabel returns a short human-readable label for the gated state
// (e.g. "gated", "gated (auto)", or "" when not gated).
func GatedLabel(gated any) string {
	if !gatedIsOn(gated) {
		return ""
	}
	switch v := gated.(type) {
	case string:
		if v != "" && v != "true" {
			return "gated (" + v + ")"
		}
	}
	return "gated"
}

// inferenceAvailable returns true if the model advertises a serverless
// inference provider via the "endpoints_compatible" / "text-generation-inference"
// / "text-embeddings-inference" tags.
func inferenceAvailable(tags []string) bool {
	for _, t := range tags {
		if t == "endpoints_compatible" || t == "text-generation-inference" || t == "text-embeddings-inference" {
			return true
		}
	}
	return false
}



// knownLibraries lists libraries defined in /api/models-tags-by-type so we
// can split language codes from library tags.
var knownLibraries = map[string]bool{
	"pytorch": true, "tf": true, "jax": true, "safetensors": true,
	"transformers": true, "peft": true, "gguf": true, "tensorboard": true,
	"diffusers": true, "onnx": true, "stable-baselines3": true,
	"sentence-transformers": true, "mlx": true, "ml-agents": true,
	"tf-keras": true, "keras": true, "joblib": true, "adapter-transformers": true,
	"transformers.js": true, "timm": true, "setfit": true, "openvino": true,
	"sample-factory": true, "flair": true, "coreml": true, "tflite": true,
	"nemo": true, "fastai": true, "espnet": true, "bertopic": true,
	"spacy": true, "rust": true, "sklearn": true, "fasttext": true,
	"open_clip": true, "executorch": true, "keras-hub": true, "asteroid": true,
	"speechbrain": true, "allennlp": true, "llamafile": true, "fairseq": true,
	"paddlepaddle": true, "paddleocr": true, "stanza": true, "pyannote-audio": true,
	"optimum_habana": true, "span-marker": true, "optimum_graphcore": true,
	"paddlenlp": true, "unity-sentis": true, "dduf": true,
}

// languageCodes returns language tag codes, stripping the "language" axis
// from the raw tag list.
func languageCodes(tags []string) []string {
	var out []string
	for _, t := range tags {
		if len(t) == 2 || len(t) == 3 {
			if isLikelyLang(t) {
				out = append(out, t)
			}
		}
	}
	sort.Strings(out)
	return out
}

func isLikelyLang(s string) bool {
	if knownLibraries[s] {
		return false
	}
	if strings.HasPrefix(s, "license:") || strings.HasPrefix(s, "arxiv:") || strings.HasPrefix(s, "doi:") || strings.HasPrefix(s, "region:") || strings.HasPrefix(s, "base_model:") {
		return false
	}
	if s == "conversational" || s == "transformers" || s == "safetensors" || s == "pytorch" {
		return false
	}
	return true
}

// pipelineLabel turns "text-generation" into "Text Generation".
func pipelineLabel(p string) string {
	if p == "" {
		return ""
	}
	parts := strings.Split(p, "-")
	for i, part := range parts {
		if part == "" {
			continue
		}
		parts[i] = strings.ToUpper(part[:1]) + part[1:]
	}
	return strings.Join(parts, " ")
}

// authorOf returns the "author" portion of a model ID (everything before the
// first "/"). For "meta-llama/Llama-3-70B-Instruct" returns "meta-llama".
func authorOf(id string) string {
	if i := strings.Index(id, "/"); i > 0 {
		return id[:i]
	}
	return id
}

func hasTag(tags []string, want string) bool {
	for _, t := range tags {
		if t == want {
			return true
		}
	}
	return false
}

func hasTagPrefix(tags []string, p string) bool {
	for _, t := range tags {
		if strings.HasPrefix(t, p) {
			return true
		}
	}
	return false
}

// ----------------------------------------------------------------------------
// Parameter size heuristics
// ----------------------------------------------------------------------------

// paramSizeRe matches "7B", "1.5B", "110M", "2.7T" style suffixes bounded
// by `-`, `/`, `_`, or end-of-string. `K` is excluded on purpose because
// "4k-instruct" / "32k-context" patterns refer to context size, not params.
var paramSizeRe = regexp.MustCompile(`(?:^|[-/])(\d+(?:\.\d+)?)([BMT])(?:[-_/]|$)`)

// ParamSize returns a best-effort parameter count (raw units: 7_000_000_000
// for "7B") extracted from a model ID and/or safetensors metadata.
//
// Heuristics, in priority order:
//  1. safetensors.total (when present on the rich endpoint)
//  2. largest safetensors variant
//  3. ID regex match — the LARGEST match wins ("Llama-3-70B-Instruct" → 70B)
//
// Returns 0 when nothing matches, which callers must treat as "unknown".
func ParamSize(m Model) int64 {
	if len(m.Safetensors) > 0 {
		if v, ok := m.Safetensors["total"]; ok {
			switch n := v.(type) {
			case float64:
				return int64(n)
			case int64:
				return n
			case int:
			return int64(n)
			}
		}
		var best int64
		for _, v := range m.Safetensors {
			if k, ok := v.(float64); ok {
				if int64(k) > best {
					best = int64(k)
				}
			}
		}
		if best > 0 {
			return best
		}
	}
	return paramSizeFromID(m.ID)
}

func paramSizeFromID(id string) int64 {
	matches := paramSizeRe.FindAllStringSubmatch(id, -1)
	if len(matches) == 0 {
		return 0
	}
	var best int64
	for _, mm := range matches {
		n, _ := strconv.ParseFloat(mm[1], 64)
		var mult int64
		switch strings.ToUpper(mm[2]) {
		case "M":
			mult = 1_000_000
		case "B":
			mult = 1_000_000_000
		case "T":
			mult = 1_000_000_000_000
		}
		v := int64(n * float64(mult))
		if v > best {
			best = v
		}
	}
	return best
}

// FormatParamSize renders an absolute param count as "7B", "1.5B", "350M", "2T".
// Returns "" for 0 / unknown.
func FormatParamSize(n int64) string {
	if n <= 0 {
		return ""
	}
	const (
		K = 1_000
		M = 1_000_000
		B = 1_000_000_000
		T = 1_000_000_000_000
	)
	switch {
	case n >= T:
		return trimFloat(float64(n)/float64(T)) + "T"
	case n >= B:
		return trimFloat(float64(n)/float64(B)) + "B"
	case n >= M:
		return trimFloat(float64(n)/float64(M)) + "M"
	case n >= K:
		return trimFloat(float64(n)/float64(K)) + "K"
	}
	return strconv.FormatInt(n, 10)
}

func trimFloat(f float64) string {
	if f == float64(int64(f)) {
		return strconv.FormatInt(int64(f), 10)
	}
	return strings.TrimRight(strings.TrimRight(fmt.Sprintf("%.2f", f), "0"), ".")
}

// ----------------------------------------------------------------------------
// Timestamps
// ----------------------------------------------------------------------------

// Created parses the model's createdAt timestamp.
func Created(m Model) time.Time {
	if m.CreatedAt == "" {
		return time.Time{}
	}
	if t, err := time.Parse(time.RFC3339Nano, m.CreatedAt); err == nil {
		return t
	}
	if t, err := time.Parse(time.RFC3339, m.CreatedAt); err == nil {
		return t
	}
	return time.Time{}
}

// Modified parses the model's lastModified timestamp.
func Modified(m Model) time.Time {
	if m.LastModified == "" {
		return time.Time{}
	}
	if t, err := time.Parse(time.RFC3339Nano, m.LastModified); err == nil {
		return t
	}
	if t, err := time.Parse(time.RFC3339, m.LastModified); err == nil {
		return t
	}
	return time.Time{}
}

// ----------------------------------------------------------------------------
// Summary — derived display fields used by cards + detail page
// ----------------------------------------------------------------------------

// Summary is the pre-computed display projection of a Model.
type Summary struct {
	License            string
	LicenseIsCopyleft  bool
	Languages          []string
	HasInference       bool
	IsGated            bool
	IsPrivate          bool
	IsDisabled         bool
	PipelineLabel      string
	LibraryLabel       string
	Author             string
	HasBaseModel       bool
	HasEvalResults     bool
	HasCarbonEmissions bool
	IsMerge            bool
	IsMoE              bool
	Has8Bit            bool
	Has4Bit            bool
	HasCustomCode      bool
	Region             string
	Created            time.Time
	Modified           time.Time
}

// Summarize returns the display projection of m.
func Summarize(m Model) Summary {
	lic := licenseOf(m.Tags)
	return Summary{
		License:            lic,
		LicenseIsCopyleft:  copyleft[lic],
		Languages:          languageCodes(m.Tags),
		HasInference:       inferenceAvailable(m.Tags),
		IsGated:            gatedIsOn(m.Gated),
		IsPrivate:          m.Private,
		IsDisabled:         m.Disabled,
		PipelineLabel:      pipelineLabel(m.PipelineTag),
		LibraryLabel:       m.LibraryName,
		Author:             authorOf(m.ID),
		HasBaseModel:       hasTagPrefix(m.Tags, "base_model:"),
		HasEvalResults:     hasTag(m.Tags, "eval-results"),
		HasCarbonEmissions: hasTag(m.Tags, "co2_eq_emissions"),
		IsMerge:            hasTag(m.Tags, "merge"),
		IsMoE:              hasTag(m.Tags, "moe"),
		Has8Bit:            hasTag(m.Tags, "8-bit"),
		Has4Bit:            hasTag(m.Tags, "4-bit"),
		HasCustomCode:      hasTag(m.Tags, "custom_code"),
		Region:             regionOf(m.Tags),
		Created:            Created(m),
		Modified:           Modified(m),
	}
}

// ----------------------------------------------------------------------------
// Description teaser (rendered on cards + detail page)
// ----------------------------------------------------------------------------

// truncateRunes truncates s to max runes, appending an ellipsis when truncated.
func truncateRunes(s string, max int) string {
	if max <= 0 {
		return s
	}
	r := []rune(s)
	if len(r) <= max {
		return s
	}
	if max > 1 {
		return string(r[:max-1]) + "…"
	}
	return string(r[:max])
}

// DescriptionTeaser extracts a 1-line teaser from the model detail. Falls
// back through three layers, in priority order:
//
//  1. md.Description (the free-text prose field on /api/models/{id})
//  2. cardData text fields ("description", "summary", "model_summary")
//  3. composed text from structured cardData fields when nothing else is
//     available — most modern HF models have no prose but DO have rich
//     metadata we can narrate ("X library · N language · trained on M").
//
// Returns "" only when even the structural fallback has nothing useful.
func DescriptionTeaser(md ModelDetail) string {
	// 1) Free-text description
	if md.Description != "" {
		s := strings.TrimSpace(md.Description)
		s = strings.TrimLeft(s, "# \t")
		if s != "" {
			return truncateRunes(s, 280)
		}
	}
	// 2) cardData text fields
	if md.CardData != nil {
		for _, k := range []string{"description", "summary", "model_summary"} {
			if v, ok := md.CardData[k]; ok {
				if s, ok := v.(string); ok {
					s = strings.TrimSpace(s)
					s = strings.TrimLeft(s, "# \t")
					if s != "" {
						return truncateRunes(s, 280)
					}
				}
			}
		}
		// 3) Composed teaser from structured cardData
		if composed := composeStructuralTeaser(md.CardData); composed != "" {
			return truncateRunes(composed, 280)
		}
	}
	return ""
}

// composeStructuralTeaser narrates the model's purpose from its cardData
// metadata. Many modern HF models (sentence-transformers, BERT, etc.) have
// no prose description but DO carry rich structured fields — pulling those
// into a one-liner gives a useful teaser where the prose path returns "".
//
// Format examples:
//   "sentence-transformers model · English · MIT license · trained on 1 dataset"
//   "transformers library · text-classification · English · Apache-2.0"
func composeStructuralTeaser(cd map[string]any) string {
	parts := make([]string, 0, 6)

	if v, ok := cd["library_name"]; ok {
		if s, ok := v.(string); ok && s != "" {
			parts = append(parts, s+" library")
		}
	}
	if v, ok := cd["pipeline_tag"]; ok {
		if s, ok := v.(string); ok && s != "" {
			parts = append(parts, pipelineLabel(s))
		}
	}
	if v, ok := cd["model_type"]; ok {
		if s, ok := v.(string); ok && s != "" {
			parts = append(parts, s+" architecture")
		}
	}
	langs := collectStrings(cd["language"])
	if len(langs) > 0 {
		if len(langs) == 1 {
			parts = append(parts, langs[0])
		} else {
			parts = append(parts, fmt.Sprintf("%d languages", len(langs)))
		}
	}
	if v, ok := cd["license"]; ok {
		if s, ok := v.(string); ok && s != "" {
			parts = append(parts, s+" license")
		}
	}
	if dss := collectStrings(cd["datasets"]); len(dss) > 0 {
		parts = append(parts, fmt.Sprintf("trained on %d dataset%s", len(dss), plural(len(dss))))
	}
	if bases := collectStrings(cd["base_model"]); len(bases) > 0 {
		parts = append(parts, fmt.Sprintf("fine-tuned from %s", bases[0]))
	}

	if len(parts) == 0 {
		return ""
	}
	return strings.Join(parts, " · ")
}

func plural(n int) string {
	if n == 1 {
		return ""
	}
	return "s"
}

func collectStrings(v any) []string {
	switch x := v.(type) {
	case nil:
		return nil
	case string:
		if x == "" {
			return nil
		}
		return []string{x}
	case []any:
		var out []string
		for _, e := range x {
			if s, ok := e.(string); ok && s != "" {
				out = append(out, s)
			}
		}
		return out
	}
	return nil
}

// CardDataPairs flattens the cardData hash into (key, value) pairs for use in
// the description block's key/value table. Nested objects/arrays are
// flattened to short comma-separated strings.
func CardDataPairs(md ModelDetail) [][2]string {
	if len(md.CardData) == 0 {
		return nil
	}
	wanted := []string{
		"license", "language", "library_name", "pipeline_tag", "tags",
		"base_model", "datasets", "model_type", "model_name",
	}
	seen := map[string]bool{}
	out := [][2]string{}
	for _, k := range wanted {
		if v, ok := md.CardData[k]; ok {
			out = append(out, [2]string{k, cardValueRender(v)})
			seen[k] = true
		}
	}
	keys := make([]string, 0, len(md.CardData))
	for k := range md.CardData {
		if !seen[k] {
			keys = append(keys, k)
		}
	}
	sort.Strings(keys)
	for _, k := range keys {
		out = append(out, [2]string{k, cardValueRender(md.CardData[k])})
	}
	return out
}

func cardValueRender(v any) string {
	switch x := v.(type) {
	case nil:
		return "—"
	case string:
		if len(x) > 200 {
			return x[:200] + "…"
		}
		return x
	case bool:
		if x {
			return "yes"
		}
		return "no"
	case float64:
		if x == float64(int64(x)) {
			return strconv.FormatInt(int64(x), 10)
		}
		return strconv.FormatFloat(x, 'g', 4, 64)
	case []any:
		if len(x) == 0 {
			return "—"
		}
		parts := make([]string, 0, len(x))
		for _, e := range x {
			parts = append(parts, cardValueRender(e))
		}
		s := strings.Join(parts, ", ")
		if len(s) > 200 {
			return s[:200] + "…"
		}
		return s
	case map[string]any:
		return "—"
	}
	return "—"
}

// ----------------------------------------------------------------------------
// Sort + filter helpers (not all used by the server today, kept for
// future client-side use)
// ----------------------------------------------------------------------------

// SortableModels returns a copy sorted by the requested key and direction.
// Unknown keys fall back to "downloads".
func SortableModels(models []Model, key string, desc bool) []Model {
	out := make([]Model, len(models))
	copy(out, models)
	sort.SliceStable(out, func(i, j int) bool {
		var less bool
		switch key {
		case "likes":
			less = out[i].Likes < out[j].Likes
		case "trendingScore":
			less = out[i].TrendingScore < out[j].TrendingScore
		case "lastModified":
			less = out[i].LastModified < out[j].LastModified
		case "createdAt":
			less = out[i].CreatedAt < out[j].CreatedAt
		default:
			less = out[i].Downloads < out[j].Downloads
		}
		if desc {
			return !less
		}
		return less
	})
	return out
}

// FilterTags returns tags matching the given prefix (without the prefix).
func FilterTags(tags []string, prefix string) []string {
	var out []string
	for _, t := range tags {
		if strings.HasPrefix(t, prefix) {
			out = append(out, strings.TrimPrefix(t, prefix))
		}
	}
	sort.Strings(out)
	return out
}
