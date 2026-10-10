#include "styles.h"

#include "texts.h"

namespace {

const QStringList iconNames { QStringLiteral("incoming"), QStringLiteral("outgoing"),
    QStringLiteral("drafts"), QStringLiteral("junk"), QStringLiteral("trash"),
    QStringLiteral("archive"), QStringLiteral("mail") };

const QStringList flagNames { QStringLiteral("dense"), QStringLiteral("dim"),
    QStringLiteral("strike"), QStringLiteral("italic") };

const QStringList colorNames { QStringLiteral("blue"), QStringLiteral("green"),
    QStringLiteral("magenta"), QStringLiteral("cyan"), QStringLiteral("yellow"),
    QStringLiteral("orange") };

const QStringList kindNames { QStringLiteral("inbox"), QStringLiteral("sent"),
    QStringLiteral("drafts"), QStringLiteral("junk"), QStringLiteral("trash"),
    QStringLiteral("archive"), QStringLiteral("other") };

}

QStringList Styles::icons() const { return iconNames; }

QStringList Styles::flags() const { return flagNames; }

QStringList Styles::colors() const { return colorNames; }

QStringList Styles::kinds() const { return kindNames; }

QStringList Styles::mailKinds() const
{
    QStringList kinds = kindNames;
    kinds.removeOne(QStringLiteral("other"));
    return kinds;
}

QString Styles::iconLabel(const QString& icon) const
{
    const qsizetype index = iconNames.indexOf(icon);
    return index < 0 ? icon : QString::fromUtf8(texts().icons[index]);
}

QString Styles::flagLabel(const QString& flag) const
{
    const qsizetype index = flagNames.indexOf(flag);
    return index < 0 ? flag : QString::fromUtf8(texts().flags[index]);
}

QString Styles::kindIcon(const QString& kind) const
{
    const qsizetype index = kindNames.indexOf(kind);
    return index < 0 ? kind : iconNames.at(index);
}

QString Styles::kindLabel(const QString& kind) const
{
    if (kind == QLatin1String("other"))
        return QString::fromUtf8(texts().kindOther);
    return iconLabel(kindIcon(kind));
}

QStringList Styles::moves(const QString& kind) const
{
    const QString inbox = QStringLiteral("inbox");
    const QString trash = QStringLiteral("trash");
    if (kind == QLatin1String("trash"))
        return { inbox };
    if (kind == QLatin1String("drafts"))
        return { trash };
    if (kind == QLatin1String("archive") || kind == QLatin1String("junk"))
        return { inbox, trash };
    return { QStringLiteral("archive"), trash };
}
