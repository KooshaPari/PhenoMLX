package server

import (
	"encoding/xml"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/KooshaPari/hfscope/internal/hfapi"
	"github.com/KooshaPari/hfscope/internal/views"
	"github.com/KooshaPari/hfscope/internal/server/templ"
)

// renderCompare renders /results/compare as full HTML.
func renderCompare(w http.ResponseWriter, r *http.Request, data views.PageData) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := templ.Compare(data).Render(r.Context(), w); err != nil {
		http.Error(w, "render error", http.StatusInternalServerError)
	}
}

// renderFeed renders /feed as HTML landing page for the RSS feed.
func renderFeed(w http.ResponseWriter, r *http.Request, data views.PageData) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := templ.Feed(data).Render(r.Context(), w); err != nil {
		http.Error(w, "render error", http.StatusInternalServerError)
	}
}

// rssFeed is the XML-encoded shape of an RSS 2.0 item.
type rssFeed struct {
	XMLName xml.Name    `xml:"rss"`
	Version string      `xml:"version,attr"`
	Channel rssChannel  `xml:"channel"`
}

type rssChannel struct {
	Title       string    `xml:"title"`
	Link        string    `xml:"link"`
	Description string    `xml:"description"`
	Language    string    `xml:"language,omitempty"`
	LastBuild   string    `xml:"lastBuildDate"`
	Items       []rssItem `xml:"item"`
}

type rssItem struct {
	Title       string `xml:"title"`
	Link        string `xml:"link"`
	Description string `xml:"description"`
	GUID        string `xml:"guid"`
	PubDate     string `xml:"pubDate"`
	Author      string `xml:"author,omitempty"`
}

// renderFeedXML builds the RSS 2.0 XML body for the given models + filter state.
// Caller has already validated up to 50 items.
func renderFeedXML(r *http.Request, params hfapi.SearchParams, models []hfapi.Model, hfBase string) string {
	base := feedBaseURL(r)
	feedTitle := "HFScope — " + feedStateDescription(params)
	feedLink := base + feedQueryString(r, "")
	feedDesc := "Latest HuggingFace models matching: " + feedStateDescription(params)

	// hfBase is already a full URL like "https://huggingface.co"
	pubBase := hfBase
	out := rssFeed{
		Version: "2.0",
		Channel: rssChannel{
			Title:       feedTitle,
			Link:        feedLink,
			Description: feedDesc,
			Language:    "en",
			LastBuild:   time.Now().UTC().Format(http.TimeFormat),
		},
	}

	for _, m := range models {
		link := pubBase + "/" + m.ID
		desc := m.ID
		if m.PipelineTag != "" {
			desc = m.PipelineTag + " — " + desc
		}
		if m.LibraryName != "" {
			desc = desc + " · library=" + m.LibraryName
		}
		out.Channel.Items = append(out.Channel.Items, rssItem{
			Title:       m.ID,
			Link:        link,
			Description: desc,
			GUID:        link,
			PubDate:     formatRSSDate(m.CreatedAt),
			Author:      m.Author,
		})
	}

	body, err := xml.MarshalIndent(out, "", "  ")
	if err != nil {
		return `<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>HFScope — feed error</title></channel></rss>`
	}
	return xml.Header + string(body)
}

// feedBaseURL returns the scheme+host portion of the current request URL.
func feedBaseURL(r *http.Request) string {
	scheme := "http"
	if r.TLS != nil {
		scheme = "https"
	}
	if h := r.Header.Get("X-Forwarded-Proto"); h != "" {
		scheme = h
	}
	host := r.Host
	return scheme + "://" + host
}

// feedQueryString returns "?key=value&..." for the URL search params, with
// an optional override suffix (e.g. "" for the channel link, or "_escaped_" for items).
func feedQueryString(r *http.Request, suffix string) string {
	q := r.URL.Query().Encode()
	if q == "" {
		return ""
	}
	out := "?" + q
	if suffix != "" {
		out = out + "&" + suffix
	}
	return out
}

// feedStateDescription returns a human-readable summary of the current filter
// state, suitable for the feed's title/description.
func feedStateDescription(p hfapi.SearchParams) string {
	parts := []string{}
	if p.Search != "" {
		parts = append(parts, fmt.Sprintf("q=%q", p.Search))
	}
	if p.PipelineTag != "" {
		parts = append(parts, "task="+p.PipelineTag)
	}
	if p.Library != "" {
		parts = append(parts, "library="+p.Library)
	}
	if p.Language != "" {
		parts = append(parts, "language="+p.Language)
	}
	if p.License != "" {
		parts = append(parts, "license="+p.License)
	}
	if p.Author != "" {
		parts = append(parts, "author="+p.Author)
	}
	if p.Gated != "" {
		parts = append(parts, "gated="+p.Gated)
	}
	if len(p.Filter) > 0 {
		parts = append(parts, "filter="+strings.Join(p.Filter, ","))
	}
	if len(parts) == 0 {
		return "browse mode"
	}
	return strings.Join(parts, " · ")
}

// formatRSSDate parses an RFC3339 timestamp and emits RFC1123 for RSS pubDate.
// Falls back to the raw string on parse error.
func formatRSSDate(s string) string {
	if s == "" {
		return ""
	}
	for _, layout := range []string{time.RFC3339, time.RFC3339Nano, "2006-01-02T15:04:05.000Z", "2006-01-02"} {
		if t, err := time.Parse(layout, s); err == nil {
			return t.UTC().Format(http.TimeFormat)
		}
	}
	return s
}
