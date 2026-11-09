context ctx is
    use work.pkg_a.all;
end context ctx;

package pkg_a is
    type t_state is (a, b);
end package pkg_a;

use context work.ctx;

entity top is
    port ( x : out t_state );
end entity top;