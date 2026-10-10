#include "localfiles.h"

#include "text.h"

#include <QFileInfo>
#include <QVariantMap>

QVariantList LocalFiles::describe(const QList<QUrl>& urls) const
{
    QVariantList files;
    for (const QUrl& url : urls) {
        if (!url.isLocalFile())
            continue;
        const QFileInfo file(url.toLocalFile());
        if (!file.isFile())
            continue;
        files.append(QVariantMap {
            { QStringLiteral("path"), file.absoluteFilePath() },
            { QStringLiteral("name"), file.fileName() },
            { QStringLiteral("size"), fileSize(file.size()) },
        });
    }
    return files;
}
