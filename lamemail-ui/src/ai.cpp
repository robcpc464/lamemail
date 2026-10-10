#include "ai.h"

#include "lamemail.h"

#include <QDebug>
#include <QVariantMap>

void Ai::reload()
{
    lamemail::run(this, { QStringLiteral("ai"), QStringLiteral("show"), QStringLiteral("--json") },
        [this](const lamemail::Outcome& outcome) {
            QString error;
            const auto setup
                = outcome.ok ? aiSetupFromJson(outcome.out.trimmed(), &error) : std::nullopt;
            if (!setup) {
                qWarning().noquote() << "lamemail-ui:" << (outcome.ok ? error : outcome.error);
                return;
            }
            if (*setup == m_setup)
                return;
            m_setup = *setup;
            emit changed();
        });
}

QVariantList Ai::programs() const
{
    QVariantList programs;
    for (const AiProgram& program : m_setup.programs)
        programs.append(QVariantMap {
            { QStringLiteral("program"), program.program },
            { QStringLiteral("name"), program.name },
            { QStringLiteral("url"), program.url },
            { QStringLiteral("found"), program.found },
            { QStringLiteral("running"), program.running },
            { QStringLiteral("command"), program.command },
            { QStringLiteral("ollama"), program.ollama },
        });
    return programs;
}

void Ai::choose(const QString& choice, const QString& value)
{
    QStringList command { QStringLiteral("ai"), QStringLiteral("set"), choice };
    if (!value.trimmed().isEmpty())
        command << QStringLiteral("--") << value.trimmed();
    change(command);
}

void Ai::remove() { change({ QStringLiteral("ai"), QStringLiteral("remove") }); }

void Ai::setContextLimit(int chars)
{
    change({ QStringLiteral("ai"), QStringLiteral("context-limit"), QString::number(chars) });
}

void Ai::change(const QStringList& command)
{
    lamemail::run(this, command, [this](const lamemail::Outcome& outcome) {
        setError(outcome.ok ? QString() : outcome.error);
        reload();
    });
}

void Ai::setError(const QString& error)
{
    if (m_error == error)
        return;
    m_error = error;
    emit errorChanged();
}
