#include "folders.h"

#include "lamemail.h"

int Folders::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : int(m_rows.size());
}

QVariant Folders::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.row() >= m_rows.size())
        return { };
    const Folder& row = m_rows.at(index.row());
    const Overrides& set = row.set;
    switch (role) {
    case KindRole:
        return row.kind;
    case IconRole:
        return row.style.icon;
    case DenseRole:
        return row.style.dense;
    case DimRole:
        return row.style.dim;
    case StrikeRole:
        return row.style.strike;
    case ItalicRole:
        return row.style.italic;
    case IndentRole:
        return row.style.indent;
    case ColorRole:
        return row.style.color;
    case IconSetRole:
        return set.icon.has_value();
    case DenseSetRole:
        return set.dense.has_value();
    case DimSetRole:
        return set.dim.has_value();
    case StrikeSetRole:
        return set.strike.has_value();
    case ItalicSetRole:
        return set.italic.has_value();
    case IndentSetRole:
        return set.indent.has_value();
    case ColorSetRole:
        return set.color.has_value();
    case AnySetRole:
        return !set.isEmpty();
    case MessagesRole:
        return row.messages;
    case UnreadRole:
        return row.unread;
    case ExistsRole:
        return !row.accounts.isEmpty();
    default:
        return { };
    }
}

QHash<int, QByteArray> Folders::roleNames() const
{
    return {
        { KindRole, "kind" },
        { IconRole, "icon" },
        { DenseRole, "dense" },
        { DimRole, "dim" },
        { StrikeRole, "strike" },
        { ItalicRole, "italic" },
        { IndentRole, "indent" },
        { ColorRole, "color" },
        { IconSetRole, "iconSet" },
        { DenseSetRole, "denseSet" },
        { DimSetRole, "dimSet" },
        { StrikeSetRole, "strikeSet" },
        { ItalicSetRole, "italicSet" },
        { IndentSetRole, "indentSet" },
        { ColorSetRole, "colorSet" },
        { AnySetRole, "anySet" },
        { MessagesRole, "messages" },
        { UnreadRole, "unread" },
        { ExistsRole, "exists" },
    };
}

void Folders::reload()
{
    if (m_loading) {
        m_pending = true;
        return;
    }
    m_pending = false;
    setLoading(true);
    lamemail::run(this,
        { QStringLiteral("folder"), QStringLiteral("list"), QStringLiteral("--json") },
        [this](const lamemail::Outcome& outcome) {
            if (outcome.ok) {
                replace(fromJsonLines(outcome.out, folderFromJson));
                setError({ });
            } else {
                setError(outcome.error);
            }
            setLoading(false);
            if (m_pending)
                reload();
        });
}

void Folders::set(const QString& kind, const QString& property, const QString& value)
{
    change({ QStringLiteral("folder"), QStringLiteral("set"), kind, property, value });
}

void Folders::reset(const QString& kind)
{
    change({ QStringLiteral("folder"), QStringLiteral("reset"), kind });
}

void Folders::change(const QStringList& command)
{
    m_changes.append(command);
    if (m_changes.size() == 1)
        runChange();
}

void Folders::runChange()
{
    lamemail::run(this, m_changes.first(), [this](const lamemail::Outcome& outcome) {
        m_changes.removeFirst();
        if (!outcome.ok)
            setError(outcome.error);
        if (!m_changes.isEmpty())
            runChange();
        else if (outcome.ok)
            reload();
    });
}

void Folders::replace(QList<Folder> rows)
{
    bool sameKinds = rows.size() == m_rows.size();
    for (qsizetype i = 0; sameKinds && i < rows.size(); ++i)
        sameKinds = rows.at(i).kind == m_rows.at(i).kind;
    if (sameKinds) {
        m_rows = std::move(rows);
        if (!m_rows.isEmpty())
            emit dataChanged(index(0), index(int(m_rows.size()) - 1));
        return;
    }
    beginResetModel();
    m_rows = std::move(rows);
    endResetModel();
}

void Folders::setLoading(bool loading)
{
    if (m_loading == loading)
        return;
    m_loading = loading;
    emit loadingChanged();
}

void Folders::setError(const QString& error)
{
    if (m_error == error)
        return;
    m_error = error;
    emit errorChanged();
}
