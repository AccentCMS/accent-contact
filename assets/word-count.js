/**
 * Word count island component (accent-contact plugin)
 *
 * Counts words in the page content and displays the count.
 * Demonstrates a plugin-provided island component.
 */
if (window.AccentIslands) {
window.AccentIslands.register("word-count", function (el, props) {
    var target = props.target || "article";
    var article = document.querySelector(target);
    if (!article) return;

    var text = article.textContent || "";
    var words = text.trim().split(/\s+/).filter(function (w) { return w.length > 0; }).length;

    el.textContent = words.toLocaleString() + " words";
});
}
