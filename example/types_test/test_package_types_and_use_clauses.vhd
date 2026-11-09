package pkg_a is
    type t_state is (s0, s1, s2);
end package pkg_a;

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;

use work.pkg_a.all;

package pkg_b is
    type t_rec is record
        state : t_state;
        data  : std_logic_vector(7 downto 0);
    end record;
    subtype t_sub is t_rec;
end package pkg_b;

use work.pkg_b.all;

entity top is
    port (
        x : out t_sub;
        y : out pkg_b.t_rec;
        z : out work.pkg_a.t_state
    );
end entity top;
