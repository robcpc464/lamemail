#include "manual.h"

#include "lamemail.h"

#include <QJsonDocument>
#include <QJsonObject>

void Manual::load()
{
    lamemail::run(this, { QStringLiteral("commands"), QStringLiteral("--json") },
        [this](const lamemail::Outcome& outcome) {
            QVariantList commands;
            for (const QByteArray& line : outcome.out.split('\n')) {
                const QJsonObject command = QJsonDocument::fromJson(line).object();
                if (!command.isEmpty())
                    commands.append(command.toVariantMap());
            }
            m_commands = commands;
            m_error = outcome.ok ? QString() : outcome.error;
            emit changed();
        });
}
