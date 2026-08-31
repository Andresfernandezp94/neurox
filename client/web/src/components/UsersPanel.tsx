// UsersPanel — admin-only CRUD UI for /v1/users.
// EP-0007. Only visible to Admin role.
// Migrado al look and feel de MCP/General/Env (sesión 2026-08-14):
//   - Container con padding y gap consistente
//   - Header con título + Refresh button (icon)
//   - Cards por usuario (no tabla HTML)
//   - Iconos en section header + actions (icon-only buttons)
//   - Form de create como Card toggle (similar a Env categories)
//   - Edit role inline con select + Save/Cancel

import { useCallback, useEffect, useState } from "react";
import {
  listUsers,
  createUser,
  updateUser,
  deleteUser,
  type UserInfo,
  type Role,
} from "../api/auth";
import { ApiError } from "../api/client";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { Button } from "../shared/components/atoms/Button";
import { Input } from "../shared/components/atoms/Input";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { Badge } from "../shared/components/atoms/Badge";
import {
  IconClose,
  IconEdit,
  IconLoop,
  IconPlus,
  IconTrash,
  IconUsers,
} from "../shared/components/Icons";

const ROLES: Role[] = ["Admin", "Operator", "Viewer"];

function generatePassword(len = 16): string {
  const charset =
    "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789!@#$%^&*";
  const arr = new Uint32Array(len);
  crypto.getRandomValues(arr);
  return Array.from(arr, (n) => charset[n % charset.length]).join("");
}

interface DraftUser {
  username: string;
  password: string;
  role: Role;
}

const EMPTY_DRAFT: DraftUser = { username: "", password: "", role: "Viewer" };

function roleVariant(role: Role): "danger" | "warn" | "neutral" {
  if (role === "Admin") return "danger";
  if (role === "Operator") return "warn";
  return "neutral";
}

export function UsersPanel(): React.JSX.Element {
  const [users, setUsers] = useState<UserInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [draft, setDraft] = useState<DraftUser>(EMPTY_DRAFT);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingRole, setEditingRole] = useState<Role>("Viewer");
  const [pending, setPending] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await listUsers();
      setUsers(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleCreate = useCallback(async () => {
    setError(null);
    setPending("create");
    try {
      await createUser(draft.username, draft.password, draft.role);
      setDraft(EMPTY_DRAFT);
      setShowForm(false);
      await refresh();
    } catch (e) {
      const msg =
        e instanceof ApiError
          ? e.message
          : e instanceof Error
            ? e.message
            : String(e);
      setError(msg);
    } finally {
      setPending(null);
    }
  }, [draft, refresh]);

  const handleSaveRole = useCallback(
    async (id: string) => {
      setError(null);
      setPending(`role:${id}`);
      try {
        await updateUser(id, { role: editingRole });
        setEditingId(null);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setPending(null);
      }
    },
    [editingRole, refresh],
  );

  const handleResetPwd = useCallback(async (id: string) => {
    const newPwd = generatePassword();
    setError(null);
    try {
      await updateUser(id, { password: newPwd });
      // TODO: reemplazar alert() por modal copiable con el password generado.
      alert(
        `New password (copy now, it won't be shown again):\n\n${newPwd}`,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const handleDelete = useCallback(
    async (id: string, username: string) => {
      if (!confirm(`Delete user "${username}"? This cannot be undone.`)) return;
      setError(null);
      setPending(`delete:${id}`);
      try {
        await deleteUser(id);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setPending(null);
      }
    },
    [refresh],
  );

  return (
    <Stack gap="md" className="users-panel" data-testid="config-users-tab">
      {error && <ErrorBanner variant="error">{error}</ErrorBanner>}

      <Row justify="between" align="center">
        <h2 className="strong text-lg users-panel__title">Users</h2>
        <Row gap="sm" align="center">
          <Button
            variant="secondary"
            size="sm"
            onClick={() => void refresh()}
            disabled={loading}
          >
            <IconLoop /> Refresh
          </Button>
          <Button
            variant="primary"
            size="sm"
            onClick={() => setShowForm((v) => !v)}
            data-testid="users-new-toggle"
          >
            {showForm ? <IconClose /> : <IconPlus />} {showForm ? "Cancel" : "New user"}
          </Button>
        </Row>
      </Row>

      {showForm && (
        <Card className="users-panel__form">
          <Stack gap="sm">
            <Row gap="sm" align="center" className="users-panel__form-header">
              <IconPlus />
              <h3 className="users-panel__form-title">New user</h3>
            </Row>
            <Row gap="sm" align="center">
              <label className="users-panel__label">Username</label>
              <Input
                value={draft.username}
                onChange={(e) =>
                  setDraft({ ...draft, username: e.target.value })
                }
                data-testid="users-new-username"
              />
            </Row>
            <Row gap="sm" align="center">
              <label className="users-panel__label">Password</label>
              <Input
                type="text"
                value={draft.password}
                placeholder="(auto-generate)"
                onChange={(e) =>
                  setDraft({ ...draft, password: e.target.value })
                }
                data-testid="users-new-password"
                className="users-panel__input-grow"
              />
              <Button
                variant="secondary"
                onClick={() =>
                  setDraft({ ...draft, password: generatePassword() })
                }
                data-testid="users-new-genpwd"
              >
                Generate
              </Button>
            </Row>
            <Row gap="sm" align="center">
              <label className="users-panel__label">Role</label>
              <select
                value={draft.role}
                onChange={(e) =>
                  setDraft({ ...draft, role: e.target.value as Role })
                }
                className="users-panel__select"
                data-testid="users-new-role"
              >
                {ROLES.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
            </Row>
            <Row justify="end" gap="sm">
              <Button
                variant="secondary"
                onClick={() => {
                  setDraft(EMPTY_DRAFT);
                  setShowForm(false);
                }}
              >
                <IconClose /> Cancel
              </Button>
              <Button
                variant="primary"
                onClick={() => void handleCreate()}
                disabled={
                  !draft.username || !draft.password || pending === "create"
                }
                data-testid="users-new-submit"
              >
                {pending === "create" ? "Creating…" : "Create"}
              </Button>
            </Row>
          </Stack>
        </Card>
      )}

      {loading ? (
        <p className="muted">Loading users…</p>
      ) : users.length === 0 ? (
        <p className="muted">No users yet.</p>
      ) : (
        <Stack gap="sm">
          {users.map((u) => (
            <Card key={u.id} className="users-panel__card" data-testid={`users-row-${u.username}`}>
              <Row justify="between" align="center" gap="sm">
                <Row gap="sm" align="center" className="users-panel__user-info">
                  <span className="users-panel__item-icon" aria-hidden="true">
                    <IconUsers />
                  </span>
                  <strong className="users-panel__username">{u.username}</strong>
                  {editingId === u.id ? (
                    <select
                      value={editingRole}
                      onChange={(e) =>
                        setEditingRole(e.target.value as Role)
                      }
                      className="users-panel__select users-panel__select--inline"
                    >
                      {ROLES.map((r) => (
                        <option key={r} value={r}>
                          {r}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <Badge variant={roleVariant(u.role)}>{u.role}</Badge>
                  )}
                </Row>

                <Row gap="sm" align="center" className="users-panel__meta">
                  <span className="muted users-panel__meta-item">
                    Created {new Date(u.created_at).toLocaleDateString()}
                  </span>
                  <span className="muted users-panel__meta-item">
                    Last login{" "}
                    {u.last_login_at
                      ? new Date(u.last_login_at).toLocaleString()
                      : "—"}
                  </span>
                </Row>

                <Row gap="sm" align="center" className="users-panel__actions">
                  {editingId === u.id ? (
                    <>
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={() => void handleSaveRole(u.id)}
                        disabled={pending === `role:${u.id}`}
                      >
                        <IconEdit /> Save
                      </Button>
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={() => setEditingId(null)}
                      >
                        <IconClose /> Cancel
                      </Button>
                    </>
                  ) : (
                    <>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => {
                          setEditingId(u.id);
                          setEditingRole(u.role);
                        }}
                        data-testid={`users-edit-${u.username}`}
                        title="Edit role"
                        aria-label="Edit role"
                      >
                        <IconEdit />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => void handleResetPwd(u.id)}
                        data-testid={`users-resetpwd-${u.username}`}
                        title="Reset password"
                        aria-label="Reset password"
                      >
                        <IconLoop />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => void handleDelete(u.id, u.username)}
                        data-testid={`users-delete-${u.username}`}
                        title="Delete user"
                        aria-label="Delete user"
                      >
                        <IconTrash />
                      </Button>
                    </>
                  )}
                </Row>
              </Row>
            </Card>
          ))}
        </Stack>
      )}
    </Stack>
  );
}
