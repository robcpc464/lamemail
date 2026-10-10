#include "folding.h"

#include <QTextCursor>
#include <QTextDocument>
#include <QVariantMap>

namespace {

bool quoted(const QTextBlock& block) { return block.text().trimmed().startsWith(QLatin1Char('>')); }

bool blank(const QTextBlock& block) { return block.text().trimmed().isEmpty(); }

}

void Folding::setDocument(QQuickTextDocument* document)
{
    if (m_document == document)
        return;
    if (QTextDocument* old = text())
        disconnect(old, nullptr, this, nullptr);
    m_document = document;
    if (QTextDocument* current = text())
        connect(current, &QTextDocument::contentsChanged, this, &Folding::refresh);
    emit documentChanged();
    refresh();
}

QTextDocument* Folding::text() const { return m_document ? m_document->textDocument() : nullptr; }

QList<Folding::Fold> Folding::find() const
{
    QList<Fold> found;
    const QTextDocument* document = text();
    if (!document)
        return found;
    QTextBlock written;
    QTextBlock block = document->begin();
    while (block.isValid()) {
        if (!quoted(block)) {
            if (!blank(block))
                written = block;
            block = block.next();
            continue;
        }
        QTextBlock last = block;
        for (QTextBlock next = block.next(); next.isValid(); next = next.next()) {
            if (quoted(next))
                last = next;
            else if (!blank(next) || !next.next().isValid() || !quoted(next.next()))
                break;
        }
        const bool attributed
            = written.isValid() && written.text().trimmed().endsWith(QLatin1Char(':'));
        const QTextBlock header = attributed ? written : block;
        if (header.next().isValid() && header.next().blockNumber() <= last.blockNumber())
            found.append({ header, header.next(), last });
        written = QTextBlock();
        block = last.next();
    }
    return found;
}

std::optional<Folding::Fold> Folding::at(int position) const
{
    const QTextDocument* document = text();
    if (!document)
        return std::nullopt;
    const int number = document->findBlock(position).blockNumber();
    for (const Fold& fold : find()) {
        if (number >= fold.header.blockNumber() && number <= fold.last.blockNumber())
            return fold;
    }
    return std::nullopt;
}

void Folding::toggle(int position)
{
    if (const auto fold = at(position))
        setFolded(*fold, fold->first.isVisible());
}

void Folding::fold(int position)
{
    if (const auto fold = at(position))
        setFolded(*fold, true);
}

void Folding::foldAll()
{
    for (const Fold& fold : find())
        setFolded(fold, true);
}

void Folding::reveal(int position)
{
    const QTextDocument* document = text();
    if (!document || document->findBlock(position).isVisible())
        return;
    if (const auto fold = at(position))
        setFolded(*fold, false);
}

void Folding::replace(const QString& replacement)
{
    QTextDocument* document = text();
    if (!document)
        return;
    QTextCursor cursor(document);
    cursor.beginEditBlock();
    cursor.select(QTextCursor::Document);
    cursor.insertText(replacement);
    cursor.endEditBlock();
    foldAll();
}

void Folding::setFolded(const Fold& fold, bool folded)
{
    QTextDocument* document = text();
    for (QTextBlock block = fold.first; block.isValid(); block = block.next()) {
        block.setVisible(!folded);
        if (block == fold.last)
            break;
    }
    const int start = fold.first.position();
    const int length = fold.last.position() + fold.last.length() - start;
    document->markContentsDirty(start, length);
    emit document->contentsChange(start, length, length);
    emit document->contentsChanged();
}

void Folding::refresh()
{
    QVariantList folds;
    for (const Fold& fold : find()) {
        folds.append(QVariantMap {
            { QStringLiteral("position"), fold.header.position() },
            { QStringLiteral("end"), fold.header.position() + fold.header.length() - 1 },
            { QStringLiteral("lines"), fold.last.blockNumber() - fold.first.blockNumber() + 1 },
            { QStringLiteral("folded"), !fold.first.isVisible() },
        });
    }
    if (folds == m_folds)
        return;
    m_folds = folds;
    emit foldsChanged();
}
